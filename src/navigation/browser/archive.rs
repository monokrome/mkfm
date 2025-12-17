//! Archive browsing methods

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::filesystem::{self, ArchiveEntry, Entry};

use super::Browser;

impl Browser {
    pub fn in_archive(&self) -> bool {
        self.archive_path.is_some()
    }

    pub fn get_archive_path(&self) -> Option<&Path> {
        self.archive_path.as_deref()
    }

    pub fn get_archive_prefix(&self) -> &str {
        &self.archive_prefix
    }

    pub fn enter_archive(&mut self, archive_path: &Path) {
        self.archive_entries = filesystem::list_archive(archive_path);
        self.archive_path = Some(archive_path.to_path_buf());
        self.archive_prefix.clear();
        self.refresh();
        self.cursor = 0;
    }

    pub fn exit_archive(&mut self) {
        self.archive_path = None;
        self.archive_prefix.clear();
        self.archive_entries.clear();
        self.refresh();
    }

    pub fn enter_archive_directory(&mut self, entry: &Entry) -> bool {
        if entry.name == ".." {
            if let Some(pos) = self.archive_prefix.rfind('/') {
                self.archive_prefix = self.archive_prefix[..pos].to_string();
            } else {
                self.archive_prefix.clear();
            }
            self.refresh();
            self.cursor = 0;
            return true;
        }
        if entry.is_dir {
            if self.archive_prefix.is_empty() {
                self.archive_prefix = entry.name.clone();
            } else {
                self.archive_prefix = format!("{}/{}", self.archive_prefix, entry.name);
            }
            self.refresh();
            self.cursor = 0;
            return true;
        }
        false
    }

    pub fn parent_archive_directory(&mut self) -> bool {
        if self.archive_prefix.is_empty() {
            self.exit_archive();
            return true;
        }
        if let Some(pos) = self.archive_prefix.rfind('/') {
            self.archive_prefix = self.archive_prefix[..pos].to_string();
        } else {
            self.archive_prefix.clear();
        }
        self.refresh();
        true
    }

    pub fn refresh_archive(&mut self) {
        let prefix = self.archive_prefix.clone();
        self.all_entries = build_archive_entries(&self.archive_entries, &prefix);
        Self::sort_entries_impl(&mut self.all_entries, self.sort_mode, self.sort_reverse);
        self.apply_filter();
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }
}

fn build_archive_entries(archive_entries: &[ArchiveEntry], prefix: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut seen_dirs: HashSet<String> = HashSet::new();

    if !prefix.is_empty() {
        entries.push(Entry {
            name: "..".to_string(),
            path: PathBuf::from(".."),
            is_dir: true,
            size: 0,
            modified: None,
            depth: 0,
        });
    }

    for entry in archive_entries {
        if let Some(new_entry) = process_archive_entry(entry, prefix, &mut seen_dirs) {
            entries.push(new_entry);
        }
    }

    entries
}

fn process_archive_entry(
    entry: &ArchiveEntry,
    prefix: &str,
    seen_dirs: &mut HashSet<String>,
) -> Option<Entry> {
    let entry_path = &entry.full_path;
    if !prefix.is_empty() && !entry_path.starts_with(prefix) {
        return None;
    }

    let relative = if prefix.is_empty() {
        entry_path.as_str()
    } else {
        entry_path
            .strip_prefix(prefix)
            .and_then(|s| s.strip_prefix('/'))
            .unwrap_or("")
    };

    if relative.is_empty() {
        return None;
    }

    let parts: Vec<&str> = relative.split('/').collect();
    if parts.is_empty() {
        return None;
    }

    let name = parts[0];
    let is_intermediate_dir = parts.len() > 1;

    if is_intermediate_dir {
        create_dir_entry_if_new(name, entry_path, seen_dirs)
    } else if entry.is_dir {
        create_dir_entry_if_new(name, &entry.full_path, seen_dirs)
    } else {
        Some(Entry {
            name: name.to_string(),
            path: PathBuf::from(&entry.full_path),
            is_dir: false,
            size: entry.size,
            modified: None,
            depth: 0,
        })
    }
}

fn create_dir_entry_if_new(
    name: &str,
    path: &str,
    seen_dirs: &mut HashSet<String>,
) -> Option<Entry> {
    if seen_dirs.insert(name.to_string()) {
        Some(Entry {
            name: name.to_string(),
            path: PathBuf::from(path),
            is_dir: true,
            size: 0,
            modified: None,
            depth: 0,
        })
    } else {
        None
    }
}
