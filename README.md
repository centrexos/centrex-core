# centrex_core

Rust-based core engine for CentrexOS. Extracts a local rootfs archive, finalizes the base filesystem layout, and provides a packaging engine for ELF binary patching and DNF metadata parsing.

## Modules

| Module | Responsibility |
| -------- | --------------- |
| `bootstrapper` | Unpacks a `.tar.xz` rootfs archive to a target directory and finalizes the layout (removes upstream package manager binaries, writes `os-release`, creates the distro store path) |
| `translator` | Parses gzip-compressed DNF XML repository metadata and patches ELF binary RUNPATH entries |

## Structure

```text
core/
├── src/
│   ├── main.rs          # Entry point — drives bootstrapper then initializes packaging engine
│   ├── bootstrapper.rs  # CoreBootstrapper: rootfs extraction + layout finalization
│   └── translator.rs    # PackagingEngine: DNF metadata parser + ELF rpath patcher
├── rootfs.tar.xz        # Base rootfs archive (Fedora Core base)
└── Cargo.toml
```

## Dependencies

| Crate | Use |
| ------- | ----- |
| `tar` / `xz2` | Extract `.tar.xz` rootfs archive |
| `flate2` | Decompress gzip-wrapped DNF XML metadata |
| `roxmltree` | Parse DNF repository XML |
| `elb` | Read and patch ELF binary dynamic section (RUNPATH) |
| `reqwest` | HTTP client (reserved for future remote metadata fetching) |

## Usage

```sh
# Check — resolve dependencies and typecheck
cargo check

# Build
cargo build --release

# Run — pass path to a local rootfs .tar.xz archive
cargo run -- /path/to/rootfs.tar.xz
```

The engine extracts the archive to `/tmp/local_glibc_core` on first run. Subsequent runs detect the existing layout and skip extraction.
