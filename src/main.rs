mod bootstrapper;
mod translator;

use std::env;
use std::path::Path;
use std::process;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Local RootFS Glibc Core Engine ===");

    // Collect command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Error: Please provide the path to your local rootfs archive.");
        eprintln!("Usage: cargo run -- <path_to_rootfs.tar.gz>");
        process::exit(1);
    }

    let local_archive_path = &args[1];
    let core_target = "/tmp/local_glibc_core";
    let bootstrapper = bootstrapper::CoreBootstrapper::new(core_target);

    if !Path::new(core_target).exists() {
        println!("Core system frame empty. Deploying local extraction pipelines...");
        bootstrapper.extract_local_rootfs(local_archive_path)?;
        bootstrapper.finalize_core_layout()?;
    } else {
        println!("Core runtime footprint already detected at: {}", core_target);
    }

    let _engine = translator::PackagingEngine::new("/opt/distro_store");
    println!("System ready. Local glibc core operational.");

    Ok(())
}
