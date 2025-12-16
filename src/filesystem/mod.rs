//! Filesystem operations
//!
//! Split into modules for reduced complexity.

mod archive;
mod ops;
mod utils;

use std::cmp::Ordering;
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub use archive::{ArchiveEntry, extract_archive, extract_files_from_archive, list_archive};
pub use ops::{copy_file, create_symlink, delete, move_file};
pub use utils::{chmod, format_size, is_archive, trash};

/// Filesystem entry (file or directory)
#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub depth: u8,
}

impl Entry {
    fn from_dir_entry(entry: DirEntry) -> Option<Self> {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = path.is_dir();
        let metadata = entry.metadata().ok();
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = metadata.and_then(|m| m.modified().ok());

        Some(Self {
            name,
            path,
            is_dir,
            size,
            modified,
            depth: 0,
        })
    }

    pub fn with_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }
}

/// List directory contents
pub fn list_directory(path: &Path) -> Vec<Entry> {
    let Ok(read_dir) = fs::read_dir(path) else {
        return Vec::new();
    };

    let mut entries: Vec<Entry> = read_dir
        .filter_map(|e| e.ok())
        .filter_map(Entry::from_dir_entry)
        .collect();

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    if let Some(parent) = path.parent() {
        entries.insert(
            0,
            Entry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
                size: 0,
                modified: None,
                depth: 0,
            },
        );
    }

    entries
}
