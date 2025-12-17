//! File browser component
//!
//! Split into submodules for reduced complexity.

mod archive;
mod expansion;
mod expansion_helpers;
mod filter_search;
mod sorting;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::filesystem::{self, ArchiveEntry, Entry};
use crate::input::SortMode;

/// File browser state
pub struct Browser {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub(super) all_entries: Vec<Entry>,
    pub cursor: usize,
    pub show_hidden: bool,
    pub show_parent_entry: bool,
    pub(super) sort_mode: SortMode,
    pub(super) sort_reverse: bool,
    pub(super) filter: Option<String>,
    // Archive browsing
    pub(super) archive_path: Option<PathBuf>,
    pub(super) archive_prefix: String,
    pub(super) archive_entries: Vec<ArchiveEntry>,
    // Fold expansion
    pub(super) expanded_dirs: HashSet<PathBuf>,
}

impl Browser {
    pub fn new(show_hidden: bool, show_parent_entry: bool, start_path: Option<PathBuf>) -> Self {
        let path = match start_path {
            Some(p) if p.is_dir() => p,
            Some(p) => {
                eprintln!("Warning: {:?} is not a directory, using cwd", p);
                std::env::current_dir().unwrap_or_else(|_| Self::home_dir())
            }
            None => std::env::current_dir().unwrap_or_else(|_| Self::home_dir()),
        };

        let mut browser = Self {
            path,
            entries: Vec::new(),
            all_entries: Vec::new(),
            cursor: 0,
            show_hidden,
            show_parent_entry,
            sort_mode: SortMode::default(),
            sort_reverse: false,
            filter: None,
            archive_path: None,
            archive_prefix: String::new(),
            archive_entries: Vec::new(),
            expanded_dirs: HashSet::new(),
        };
        browser.refresh();
        browser
    }

    fn home_dir() -> PathBuf {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/"))
    }

    pub fn refresh(&mut self) {
        if self.archive_path.is_some() {
            self.refresh_archive();
        } else {
            self.refresh_directory();
        }
    }

    fn refresh_directory(&mut self) {
        self.all_entries = filesystem::list_directory(&self.path);

        if !self.show_hidden {
            self.all_entries
                .retain(|e| !e.name.starts_with('.') || (self.show_parent_entry && e.name == ".."));
        }

        Self::sort_entries_impl(&mut self.all_entries, self.sort_mode, self.sort_reverse);
        self.apply_filter();
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    pub(super) fn apply_filter(&mut self) {
        if let Some(ref pattern) = self.filter {
            let pattern_lower = pattern.to_lowercase();
            self.entries = self
                .all_entries
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&pattern_lower))
                .cloned()
                .collect();
        } else {
            self.entries = self.all_entries.clone();
        }
    }

    // Cursor movement
    pub fn move_cursor(&mut self, delta: i32) {
        let new_pos = self.cursor as i32 + delta;
        let max = self.entries.len().saturating_sub(1) as i32;
        self.cursor = new_pos.clamp(0, max) as usize;
    }

    pub fn cursor_to_top(&mut self) {
        self.cursor = 0;
    }

    pub fn cursor_to_bottom(&mut self) {
        self.cursor = self.entries.len().saturating_sub(1);
    }

    pub fn current_entry(&self) -> Option<&Entry> {
        self.entries.get(self.cursor)
    }

    // Navigation
    pub fn enter_directory(&mut self) -> bool {
        let Some(entry) = self.current_entry().cloned() else {
            return false;
        };

        if self.archive_path.is_some() {
            return self.enter_archive_directory(&entry);
        }

        if !entry.is_dir && filesystem::is_archive(&entry.path) {
            self.enter_archive(&entry.path);
            return true;
        }

        if !entry.is_dir {
            return false;
        }

        self.path = entry.path.clone();
        self.refresh();
        self.cursor = 0;
        true
    }

    pub fn parent_directory(&mut self) -> bool {
        if self.archive_path.is_some() {
            return self.parent_archive_directory();
        }

        let Some(parent) = self.path.parent() else {
            return false;
        };

        self.path = parent.to_path_buf();
        self.refresh();
        true
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.refresh();
    }

    pub fn next_directory(&mut self) {
        let start = self.cursor + 1;
        for i in start..self.entries.len() {
            if self.entries[i].is_dir {
                self.cursor = i;
                return;
            }
        }
    }

    pub fn prev_directory(&mut self) {
        if self.cursor == 0 {
            return;
        }
        for i in (0..self.cursor).rev() {
            if self.entries[i].is_dir {
                self.cursor = i;
                return;
            }
        }
    }

    pub fn navigate_to(&mut self, path: &Path) {
        if path.is_dir() {
            self.path = path.to_path_buf();
            self.refresh();
            self.cursor = 0;
        }
    }

    // Sorting
    pub fn set_sort(&mut self, mode: SortMode, reverse: bool) {
        self.sort_mode = mode;
        self.sort_reverse = reverse;
        self.refresh();
    }
}
