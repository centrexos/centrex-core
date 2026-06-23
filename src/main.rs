mod api;
mod bootstrapper;
mod translator;

use std::env;
use std::path::Path;
use std::process;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Centrex Core v{}", env!("CARGO_PKG_VERSION"));

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  centrex-core <rootfs.tar.xz>         Deploy core rootfs from archive");
        eprintln!("  centrex-core --status                Show system status");
        eprintln!("  centrex-core --daemon                Start privileged API daemon (root)");
        eprintln!("  centrex-core --api-call '<json>'     Send a call to the running daemon");
        process::exit(1);
    }

    match args[1].as_str() {
        "--status"   => cmd_status(),
        "--daemon"   => api::run_daemon(),
        "--api-call" => {
            let json = args.get(2)
                .ok_or("--api-call requires a JSON argument, e.g. '{\"cmd\":\"status\"}'")
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            api::api_call(json)
        }
        path => cmd_deploy(path),
    }
}

fn cmd_deploy(archive_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let core_target = "/tmp/centrex_core_root";
    let bootstrapper = bootstrapper::CoreBootstrapper::new(core_target);

    if !Path::new(core_target).exists() {
        log::info!("Core root absent — deploying from archive: {}", archive_path);
        bootstrapper.extract_local_rootfs(archive_path)?;
        bootstrapper.finalize_core_layout()?;
        log::info!("Core deployed to: {}", core_target);
    } else {
        log::info!("Core root already present at: {}", core_target);
    }

    let engine = translator::PackagingEngine::new("/opt/centrex_store");
    log::info!("Packaging engine ready at: {}", engine.store_root.display());

    Ok(())
}

fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    let core_root = "/tmp/centrex_core_root";
    let store     = "/opt/centrex_store";
    let socket    = api::SOCKET_PATH;

    println!("Centrex Core Status");
    println!("===================");
    println!("Core root:    {} {}", core_root,
        if Path::new(core_root).exists() { "[present]" } else { "[absent]" });
    println!("Package store: {} {}", store,
        if Path::new(store).exists() { "[present]" } else { "[absent]" });
    println!("API socket:   {} {}", socket,
        if Path::new(socket).exists() { "[running]" } else { "[not running]" });

    let os_release = Path::new(core_root).join("etc/os-release");
    if os_release.exists() {
        let content = std::fs::read_to_string(&os_release)?;
        for line in content.lines().take(3) {
            println!("  {}", line.trim());
        }
    }
    Ok(())
}
