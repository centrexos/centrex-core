use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use flate2::read::GzDecoder;
use roxmltree::Document;
use elb::{DynamicTag, Elf, ElfPatcher};

pub struct PackagingEngine {
    pub store_root: PathBuf,
}

impl PackagingEngine {
    pub fn new(store_path: &str) -> Self {
        Self { store_root: PathBuf::from(store_path) }
    }

    /// Parse DNF/RPM repomd primary.xml.gz to extract package metadata.
    pub fn parse_dnf_metadata(&self, gzip_xml_path: &Path) -> Result<Vec<RepoPackage>, Box<dyn std::error::Error>> {
        log::debug!("Parsing DNF metadata: {:?}", gzip_xml_path);

        let compressed_file = File::open(gzip_xml_path)?;
        let mut decoder = GzDecoder::new(compressed_file);
        let mut xml_content = String::new();
        decoder.read_to_string(&mut xml_content)?;

        let doc = Document::parse(&xml_content)?;
        let mut packages = Vec::new();

        for node in doc.descendants().filter(|n| n.has_tag_name("package")) {
            let name = node.descendants()
                .find(|n| n.has_tag_name("name"))
                .and_then(|n| n.text())
                .unwrap_or("")
                .to_string();

            let arch = node.descendants()
                .find(|n| n.has_tag_name("arch"))
                .and_then(|n| n.text())
                .unwrap_or("")
                .to_string();

            let version = node.descendants()
                .find(|n| n.has_tag_name("version"))
                .and_then(|n| n.attribute("ver"))
                .unwrap_or("")
                .to_string();

            let summary = node.descendants()
                .find(|n| n.has_tag_name("summary"))
                .and_then(|n| n.text())
                .unwrap_or("")
                .to_string();

            if arch == "x86_64" || arch == "noarch" {
                log::debug!("Found package: {} {}", name, version);
                packages.push(RepoPackage { name, version, arch, summary });
            }
        }

        log::info!("Parsed {} packages from DNF metadata", packages.len());
        Ok(packages)
    }

    /// Patch the RPATH/RUNPATH of an ELF binary to use Centrex store library paths.
    pub fn patch_rpath(&self, binary_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        log::debug!("Patching RPATH: {:?}", binary_path);

        let mut file = OpenOptions::new().read(true).write(true).open(binary_path)?;
        let elf = Elf::read(&mut file, 4096)?;
        let mut patcher = ElfPatcher::new(elf, file);

        // Prefer $ORIGIN-relative paths so the binary is relocatable within the store.
        let rpath = format!(
            "$ORIGIN/../lib:$ORIGIN/../lib64:{}:{}:/lib64:/lib:/usr/lib",
            self.store_root.join("lib").display(),
            self.store_root.join("lib64").display(),
        );
        let rpath_cstr = std::ffi::CString::new(rpath.as_str())?;
        patcher.set_library_search_path(DynamicTag::Runpath, &*rpath_cstr)?;
        patcher.finish()?;

        log::info!("RPATH patched: {:?}", binary_path);
        Ok(())
    }
}

#[derive(Debug)]
pub struct RepoPackage {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub summary: String,
}
