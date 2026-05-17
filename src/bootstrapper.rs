use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use tar::Archive;
use xz2::read::XzDecoder;

pub struct CoreBootstrapper {
    target_root: PathBuf,
}

impl CoreBootstrapper {
    pub fn new(target_path: &str) -> Self {
        Self { target_root: PathBuf::from(target_path) }
    }

    // Unpacks a pre-existing rootfs file from the local file system
    pub fn extract_local_rootfs(&self, archive_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(archive_path);
        if !path.exists() {
            return Err(format!("Local rootfs archive not found at: {}", archive_path).into());
        }

        println!("Reading local rootfs archive: {}", archive_path);
        let archive_file = File::open(path)?;
        
        println!("Generating localized runtime layout: {:?}", self.target_root);
        fs::create_dir_all(&self.target_root)?;

        println!("Inflating local .tar.xz filesystem matrix (This might take a minute)...");
        let xz_decoder = XzDecoder::new(archive_file);
        let mut archive = Archive::new(xz_decoder);

        // Standard tar extraction preserving file permissions
        archive.unpack(&self.target_root)?;
        Ok(())
    }

    pub fn finalize_core_layout(&self) -> io::Result<()> {
        println!("Isolating base file configurations from external software channels...");
        
        // Wipe Fedora's native package manager tracking structures to guarantee exclusive control
        let dnf_binary = self.target_root.join("usr/bin/dnf");
        if dnf_binary.exists() { let _ = fs::remove_file(dnf_binary); }
        let rpm_binary = self.target_root.join("usr/bin/rpm");
        if rpm_binary.exists() { let _ = fs::remove_file(rpm_binary); }

        let os_release_path = self.target_root.join("etc/os-release");
        let custom_metadata = br#"NAME="BespokeFedoraCoreOS"
            ID=bespokefedoracoreos
            PRETTY_NAME="Bespoke OS (Fedora Core + Rust Engine)"
            "#;
        fs::write(os_release_path, custom_metadata)?;

        let store_path = self.target_root.join("opt/distro_store");
        fs::create_dir_all(store_path)?;

        Ok(())
    }
}