// Centrex Core privileged API daemon.
//
// Runs as root, listens on a Unix socket, and dispatches package-management and
// system-administration operations on behalf of non-root container processes.
// The socket is group-owned by `centrex` (gid 1000) with mode 0660 so that
// only members of that group can connect — root is never exposed directly.
//
// Wire protocol: newline-delimited JSON.
//   Client  →  {"cmd":"pkg-install","packages":["curl"]}  LF
//   Server  →  {"ok":true,"data":{...}}                   LF

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::Command;
use std::thread;

use serde::{Deserialize, Serialize};

pub const SOCKET_PATH: &str = "/run/centrex/core.sock";

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
enum Request {
    Status,
    SysInfo,
    PkgInstall { packages: Vec<String> },
    PkgRemove  { packages: Vec<String> },
    PkgUpdate,
    PkgUpgrade,
}

#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data:  Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ApiResponse {
    fn ok(data: serde_json::Value) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, data: None, error: Some(msg.into()) }
    }
}

// ── Public entry-points ───────────────────────────────────────────────────────

/// Start the daemon (must be called as root).
pub fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    if euid() != 0 {
        return Err("centrex-core --daemon must run as root".into());
    }

    let socket_dir = Path::new("/run/centrex");
    fs::create_dir_all(socket_dir)?;

    let socket_path = Path::new(SOCKET_PATH);
    if socket_path.exists() {
        fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;

    // mode 0660: owner=root, group=centrex → non-root centrex members can connect
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o660))?;
    Command::new("chown")
        .args(["root:centrex", SOCKET_PATH])
        .status()
        .map_err(|e| format!("chown failed: {e}"))?;

    log::info!("Core API daemon listening on {SOCKET_PATH}");
    println!("[centrexos] Core API daemon ready — socket: {SOCKET_PATH}");

    for stream in listener.incoming() {
        match stream {
            Ok(s)  => { thread::spawn(move || handle_client(s)); }
            Err(e) => log::error!("accept error: {e}"),
        }
    }
    Ok(())
}

/// Send one JSON request to the running daemon and print the response.
/// Called by `centrex-core --api-call '<json>'`.
pub fn api_call(json: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(SOCKET_PATH).map_err(|_| {
        format!(
            "Cannot reach Core API — is centrex-core --daemon running?\n  Socket: {SOCKET_PATH}"
        )
    })?;

    writeln!(stream, "{json}")?;
    stream.shutdown(Shutdown::Write)?;

    let mut response = String::new();
    BufReader::new(&stream).read_line(&mut response)?;

    let trimmed = response.trim();
    println!("{trimmed}");

    // Propagate API-level errors as process errors
    let val: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("Malformed API response: {e}"))?;
    if !val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        let msg = val.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
        return Err(msg.to_string().into());
    }

    Ok(())
}

// ── Server internals ──────────────────────────────────────────────────────────

fn handle_client(stream: UnixStream) {
    let reader_stream = match stream.try_clone() {
        Ok(s)  => s,
        Err(e) => { log::error!("stream clone error: {e}"); return; }
    };
    let reader = BufReader::new(reader_stream);
    let mut writer = stream;

    for line in reader.lines() {
        let line = match line {
            Ok(l)  => l,
            Err(_) => break,
        };
        if line.trim().is_empty() { continue; }

        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => dispatch(req),
            Err(e)  => ApiResponse::err(format!("invalid request: {e}")),
        };

        let json = serde_json::to_string(&resp)
            .unwrap_or_else(|_| r#"{"ok":false,"error":"serialization error"}"#.into());
        let _ = writeln!(writer, "{json}");
    }
}

fn dispatch(req: Request) -> ApiResponse {
    match req {
        Request::Status => ApiResponse::ok(serde_json::json!({
            "version":       env!("CARGO_PKG_VERSION"),
            "core_root":     Path::new("/tmp/centrex_core_root").exists(),
            "store":         Path::new("/opt/centrex_store").exists(),
            "container":     in_container(),
        })),

        Request::SysInfo => {
            let hostname = fs::read_to_string("/etc/hostname")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".into());
            let kver = fs::read_to_string("/proc/version")
                .map(|s| s.split_whitespace().nth(2).unwrap_or("?").to_string())
                .unwrap_or_else(|_| "?".into());
            ApiResponse::ok(serde_json::json!({
                "hostname": hostname,
                "kernel":   kver,
                "arch":     std::env::consts::ARCH,
                "os":       "CentrexOS",
            }))
        }

        Request::PkgUpdate  => run_pkg(&["update"],         "update"),
        Request::PkgUpgrade => run_pkg(&["upgrade", "-y"],  "upgrade"),

        Request::PkgInstall { packages } => {
            let owned: Vec<String> = packages;
            let refs: Vec<&str>    = owned.iter().map(|s| s.as_str()).collect();
            let mut args = vec!["install", "-y", "--no-install-recommends"];
            args.extend_from_slice(&refs);
            run_pkg(&args, "install")
        }

        Request::PkgRemove { packages } => {
            let owned: Vec<String> = packages;
            let refs: Vec<&str>    = owned.iter().map(|s| s.as_str()).collect();
            let mut args = vec!["remove", "-y"];
            args.extend_from_slice(&refs);
            run_pkg(&args, "remove")
        }
    }
}

// ── Privileged package operations ─────────────────────────────────────────────

fn run_pkg(args: &[&str], op: &str) -> ApiResponse {
    let Some(pm) = detect_package_manager() else {
        return ApiResponse::err("No supported package manager found (apt-get, dnf)");
    };

    log::info!("pkg {op}: {pm} {}", args.join(" "));

    match Command::new(pm)
        .args(args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
    {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            ApiResponse::ok(serde_json::json!({ "op": op, "output": stdout }))
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            ApiResponse::err(format!("{op} failed: {stderr}"))
        }
        Err(e) => ApiResponse::err(format!("failed to spawn {pm}: {e}")),
    }
}

fn detect_package_manager() -> Option<&'static str> {
    for pm in ["apt-get", "dnf", "yum"] {
        if Command::new(pm).arg("--version").output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(pm);
        }
    }
    None
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn euid() -> u32 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(2))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(u32::MAX)
}

fn in_container() -> bool {
    Path::new("/.dockerenv").exists()
        || Path::new("/.containerenv").exists()
        || std::env::var("container").is_ok()
}
