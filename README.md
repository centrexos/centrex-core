# centrex_core

Rust-based core engine for CentrexOS. It handles rootfs bootstrapping, the centrex package store, ELF binary patching, and — as the central security boundary in container deployments — a **privileged API daemon** that lets non-root processes perform operations that require root without ever granting root shell access.

## Modules

| Module | Responsibility |
| --- | --- |
| `api` | Privileged Unix-socket API daemon and client. Runs as root, exposes package-management and system-information commands to non-root callers over a JSON protocol |
| `bootstrapper` | Unpacks a `.tar.xz` base rootfs, writes `/etc/os-release`, creates centrex store directories, and removes upstream package-manager binaries so `cxpkg` is the only package interface |
| `translator` | Parses gzip-compressed DNF XML repository metadata; patches ELF binary `RUNPATH` entries so binaries are relocatable within the centrex store |

## Structure

```text
core/
├── src/
│   ├── main.rs          Entry point — routes CLI subcommands
│   ├── api.rs           Privileged API daemon + client (Unix socket, JSON)
│   ├── bootstrapper.rs  CoreBootstrapper: rootfs extraction + layout finalization
│   └── translator.rs    PackagingEngine: DNF metadata parser + ELF rpath patcher
├── archive/             Base rootfs archives
│   └── fedora/rootfs.tar.xz
└── Cargo.toml
```

## CLI

```text
centrex-core <rootfs.tar.xz>         Deploy core rootfs from archive
centrex-core --status                Show system status (core root, store, API socket)
centrex-core --daemon                Start privileged API daemon  [must be root]
centrex-core --api-call '<json>'     Send a call to the running daemon
```

### Daemon mode

`centrex-core --daemon` must run as root. It creates a Unix socket at `/run/centrex/core.sock` with permissions `root:centrex 0660` — only processes whose effective GID is `centrex` (gid 1000) can connect. It then loops, accepting one connection per thread, until killed.

### API call mode

`centrex-core --api-call '<json>'` is the non-root client. It connects to the socket, sends the JSON request, prints the response, and exits with code 1 if the response carries `"ok": false`.

## API Protocol

Wire format: newline-delimited JSON. Each connection carries exactly one request and receives one response.

### Commands

| JSON | Description |
| --- | --- |
| `{"cmd":"status"}` | Daemon version, core-root/store presence, container flag |
| `{"cmd":"sys-info"}` | Hostname, kernel version, architecture |
| `{"cmd":"pkg-install","packages":["curl","git"]}` | Privileged package install (apt/dnf) |
| `{"cmd":"pkg-remove","packages":["curl"]}` | Privileged package remove |
| `{"cmd":"pkg-update"}` | Refresh package index |
| `{"cmd":"pkg-upgrade"}` | Upgrade all installed packages |

### Response shape

```json
{"ok": true,  "data": { ... }}
{"ok": false, "error": "descriptive error message"}
```

### Example session

```sh
# Terminal 1 — start daemon (root)
sudo centrex-core --daemon

# Terminal 2 — non-root API calls
centrex-core --api-call '{"cmd":"status"}'
centrex-core --api-call '{"cmd":"pkg-install","packages":["curl"]}'
```

## Dependencies

| Crate | Use |
| --- | --- |
| `tar` / `xz2` | Extract `.tar.xz` rootfs archive |
| `flate2` | Decompress gzip-wrapped DNF XML metadata |
| `roxmltree` | Parse DNF repository XML |
| `elb` | Read and patch ELF binary dynamic section (RUNPATH) |
| `serde` / `serde_json` | API request/response serialization |
| `reqwest` | HTTP client (reserved for future remote metadata fetching) |

## Building

```sh
cargo check                           # type-check
cargo build --release                 # production binary
cargo test                            # unit tests
```

Or via the root Makefile:

```sh
make build-core
```
