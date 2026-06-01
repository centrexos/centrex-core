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

    pub fn extract_local_rootfs(&self, archive_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(archive_path);
        if !path.exists() {
            return Err(format!("rootfs archive not found: {}", archive_path).into());
        }

        log::info!("Extracting rootfs: {}", archive_path);
        fs::create_dir_all(&self.target_root)?;

        let archive_file = File::open(path)?;
        log::debug!("Decompressing .tar.xz — this may take a moment");
        let xz_decoder = XzDecoder::new(archive_file);
        let mut archive = Archive::new(xz_decoder);
        archive.unpack(&self.target_root)?;

        log::info!("Rootfs extracted to: {:?}", self.target_root);
        Ok(())
    }

    pub fn finalize_core_layout(&self) -> io::Result<()> {
        log::info!("Finalizing Centrex core layout");

        self.write_os_release()?;
        self.create_centrex_dirs()?;
        self.disable_foreign_package_managers()?;

        log::info!("Core layout finalized");
        Ok(())
    }

    fn write_os_release(&self) -> io::Result<()> {
        let os_release = self.target_root.join("etc/os-release");
        let content = r#"NAME="CentrexOS"
ID=centrexos
PRETTY_NAME="CentrexOS (Early Development)"
VERSION="0.1.0"
VERSION_ID="0.1.0"
HOME_URL="https://centrexos.org"
SUPPORT_URL="https://github.com/centrexos"
BUG_REPORT_URL="https://github.com/centrexos/centrexos/issues"
"#;
        fs::write(os_release, content)?;
        log::debug!("Wrote /etc/os-release");
        Ok(())
    }

    fn create_centrex_dirs(&self) -> io::Result<()> {
        for dir in &[
            "opt/centrex_store",
            "etc/cxpkg",
            "var/cache/cxpkg",
            "var/lib/cxpkg",
        ] {
            fs::create_dir_all(self.target_root.join(dir))?;
            log::debug!("Created dir: {}", dir);
        }
        Ok(())
    }

    fn disable_foreign_package_managers(&self) -> io::Result<()> {
        // Remove native package manager binaries to ensure cxpkg is the sole package interface.
        // This prevents accidental direct use of the underlying backend tools by end users.
        for bin in &["usr/bin/dnf", "usr/bin/rpm", "usr/bin/apt", "usr/bin/apt-get"] {
            let full = self.target_root.join(bin);
            if full.exists() {
                fs::remove_file(&full)?;
                log::debug!("Removed foreign binary: {}", bin);
            }
        }
        Ok(())
    }
}
