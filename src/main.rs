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
        eprintln!("Usage: centrex-core <path_to_rootfs.tar.xz>");
        eprintln!("       centrex-core --status");
        process::exit(1);
    }

    if args[1] == "--status" {
        return cmd_status();
    }

    let local_archive_path = &args[1];
    let core_target = "/tmp/centrex_core_root";
    let bootstrapper = bootstrapper::CoreBootstrapper::new(core_target);

    if !Path::new(core_target).exists() {
        log::info!("Core root absent — deploying from archive: {}", local_archive_path);
        bootstrapper.extract_local_rootfs(local_archive_path)?;
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
    let store = "/opt/centrex_store";

    println!("Centrex Core Status");
    println!("===================");
    println!("Core root:    {} {}", core_root, if Path::new(core_root).exists() { "[present]" } else { "[absent]" });
    println!("Package store: {} {}", store, if Path::new(store).exists() { "[present]" } else { "[absent]" });

    let os_release = Path::new(core_root).join("etc/os-release");
    if os_release.exists() {
        let content = std::fs::read_to_string(&os_release)?;
        for line in content.lines().take(3) {
            println!("  {}", line.trim());
        }
    }
    Ok(())
}
