//! Archive operations (listing, extraction)
//!
//! Split into submodules for reduced complexity.

mod extract;
mod list;

use std::path::Path;

pub use extract::{extract_archive, extract_files_from_archive};
pub use list::list_archive;

/// Archive entry for virtual browsing
#[derive(Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub full_path: String,
    pub is_dir: bool,
    pub size: u64,
}

impl ArchiveEntry {
    pub fn new(full_path: String, is_dir: bool, size: u64) -> Self {
        let name = full_path
            .split('/')
            .next_back()
            .unwrap_or(&full_path)
            .to_string();
        Self {
            name,
            full_path,
            is_dir,
            size,
        }
    }
}

/// Detected archive format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
    TarBz2,
    TarXz,
    Tar,
    SevenZip,
    Rar,
    Unknown,
}

impl ArchiveFormat {
    pub fn detect(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        let name = path.to_string_lossy().to_lowercase();

        match ext.as_deref() {
            Some("zip") => Self::Zip,
            Some("gz") if name.ends_with(".tar.gz") || name.ends_with(".tgz") => Self::TarGz,
            Some("bz2") if name.ends_with(".tar.bz2") => Self::TarBz2,
            Some("xz") if name.ends_with(".tar.xz") => Self::TarXz,
            Some("tar") => Self::Tar,
            Some("7z") => Self::SevenZip,
            Some("rar") => Self::Rar,
            _ => Self::Unknown,
        }
    }
}
