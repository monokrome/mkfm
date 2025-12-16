//! Clipboard for file operations

use std::path::{Path, PathBuf};

use crate::filesystem;

/// Clipboard for yank/cut/paste operations
pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub is_cut: bool,
    archive_source: Option<PathBuf>,
    archive_files: Vec<String>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            is_cut: false,
            archive_source: None,
            archive_files: Vec::new(),
        }
    }

    pub fn yank(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.is_cut = false;
        self.archive_source = None;
        self.archive_files.clear();
    }

    pub fn cut(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.is_cut = true;
        self.archive_source = None;
        self.archive_files.clear();
    }

    pub fn yank_from_archive(&mut self, archive_path: PathBuf, file_paths: Vec<String>) {
        self.paths.clear();
        self.is_cut = false;
        self.archive_source = Some(archive_path);
        self.archive_files = file_paths;
    }

    pub fn is_from_archive(&self) -> bool {
        self.archive_source.is_some()
    }

    pub fn paste_to(&mut self, dest_dir: &Path) -> std::io::Result<()> {
        if let Some(ref archive_path) = self.archive_source {
            filesystem::extract_files_from_archive(archive_path, &self.archive_files, dest_dir)?;
            return Ok(());
        }

        for src in &self.paths {
            let Some(name) = src.file_name() else {
                continue;
            };
            let dest = dest_dir.join(name);

            if self.is_cut {
                filesystem::move_file(src, &dest)?;
            } else {
                filesystem::copy_file(src, &dest)?;
            }
        }

        if self.is_cut {
            self.paths.clear();
            self.is_cut = false;
        }

        Ok(())
    }
}
