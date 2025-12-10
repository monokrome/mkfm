use std::path::PathBuf;
use crate::filesystem::{self, Entry, ArchiveEntry};
use crate::input::SortMode;

pub struct Browser {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    all_entries: Vec<Entry>,  // Unfiltered entries for search
    pub cursor: usize,
    pub show_hidden: bool,
    pub show_parent_entry: bool,
    sort_mode: SortMode,
    sort_reverse: bool,
    filter: Option<String>,
    // Archive browsing state
    archive_path: Option<PathBuf>,  // The archive file being browsed
    archive_prefix: String,         // Current path within the archive
    archive_entries: Vec<ArchiveEntry>, // All entries in the archive
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

        // Apply hidden filter
        if !self.show_hidden {
            self.all_entries.retain(|e| !e.name.starts_with('.') || (self.show_parent_entry && e.name == ".."));
        }

        // Apply sorting
        Self::sort_entries_impl(&mut self.all_entries, self.sort_mode, self.sort_reverse);

        // Apply name filter if active
        self.apply_filter();

        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    fn refresh_archive(&mut self) {
        // Build entries from archive contents at current prefix
        let prefix = &self.archive_prefix;

        let mut seen_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();
        self.all_entries.clear();

        // Add parent entry if not at archive root
        if !prefix.is_empty() {
            self.all_entries.push(Entry {
                name: "..".to_string(),
                path: PathBuf::from(".."),
                is_dir: true,
                size: 0,
                modified: None,
            });
        }

        for entry in &self.archive_entries {
            // Check if entry is under current prefix
            let entry_path = &entry.full_path;
            if !prefix.is_empty() && !entry_path.starts_with(prefix) {
                continue;
            }

            // Get the relative path from current prefix
            let relative = if prefix.is_empty() {
                entry_path.as_str()
            } else {
                entry_path.strip_prefix(prefix)
                    .and_then(|s| s.strip_prefix('/'))
                    .unwrap_or("")
            };

            if relative.is_empty() {
                continue;
            }

            // Check depth - we only want immediate children
            let parts: Vec<&str> = relative.split('/').collect();
            if parts.is_empty() {
                continue;
            }

            let name = parts[0];
            let is_intermediate_dir = parts.len() > 1;

            if is_intermediate_dir {
                // This is a directory that contains deeper entries
                if seen_dirs.insert(name.to_string()) {
                    self.all_entries.push(Entry {
                        name: name.to_string(),
                        path: PathBuf::from(entry_path),
                        is_dir: true,
                        size: 0,
                        modified: None,
                    });
                }
            } else if entry.is_dir {
                if seen_dirs.insert(name.to_string()) {
                    self.all_entries.push(Entry {
                        name: name.to_string(),
                        path: PathBuf::from(&entry.full_path),
                        is_dir: true,
                        size: entry.size,
                        modified: None,
                    });
                }
            } else {
                self.all_entries.push(Entry {
                    name: name.to_string(),
                    path: PathBuf::from(&entry.full_path),
                    is_dir: false,
                    size: entry.size,
                    modified: None,
                });
            }
        }

        // Apply sorting
        Self::sort_entries_impl(&mut self.all_entries, self.sort_mode, self.sort_reverse);

        // Apply name filter if active
        self.apply_filter();

        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    fn sort_entries_impl(entries: &mut Vec<Entry>, sort_mode: SortMode, sort_reverse: bool) {
        // Keep ".." at top
        let has_parent = entries.first().map(|e| e.name == "..").unwrap_or(false);
        let start = if has_parent { 1 } else { 0 };

        let slice = &mut entries[start..];
        match sort_mode {
            SortMode::Name => slice.sort_by(|a, b| {
                // Directories first, then by name
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
            }),
            SortMode::Size => slice.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.size.cmp(&b.size),
                }
            }),
            SortMode::Date => slice.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.modified.cmp(&b.modified),
                }
            }),
            SortMode::Type => slice.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => {
                        let ext_a = a.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        let ext_b = b.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
                    }
                }
            }),
        }

        if sort_reverse {
            slice.reverse();
        }
    }

    fn apply_filter(&mut self) {
        if let Some(ref pattern) = self.filter {
            let pattern_lower = pattern.to_lowercase();
            self.entries = self.all_entries
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&pattern_lower))
                .cloned()
                .collect();
        } else {
            self.entries = self.all_entries.clone();
        }
    }

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

    pub fn enter_directory(&mut self) -> bool {
        let Some(entry) = self.current_entry().cloned() else { return false };

        // In archive mode
        if self.archive_path.is_some() {
            if entry.name == ".." {
                // Go up within archive
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
                // Navigate into directory within archive
                if self.archive_prefix.is_empty() {
                    self.archive_prefix = entry.name.clone();
                } else {
                    self.archive_prefix = format!("{}/{}", self.archive_prefix, entry.name);
                }
                self.refresh();
                self.cursor = 0;
                return true;
            }
            return false;
        }

        // Check if entry is an archive
        if !entry.is_dir && filesystem::is_archive(&entry.path) {
            self.enter_archive(&entry.path);
            return true;
        }

        if !entry.is_dir { return false; }

        self.path = entry.path.clone();
        self.refresh();
        self.cursor = 0;
        true
    }

    /// Enter archive browsing mode
    fn enter_archive(&mut self, archive_path: &PathBuf) {
        self.archive_entries = filesystem::list_archive(archive_path);
        self.archive_path = Some(archive_path.clone());
        self.archive_prefix.clear();
        self.refresh();
        self.cursor = 0;
    }

    /// Exit archive mode and return to normal browsing
    pub fn exit_archive(&mut self) {
        self.archive_path = None;
        self.archive_prefix.clear();
        self.archive_entries.clear();
        self.refresh();
    }

    /// Check if currently browsing an archive
    pub fn in_archive(&self) -> bool {
        self.archive_path.is_some()
    }

    /// Get the archive path if browsing an archive
    pub fn get_archive_path(&self) -> Option<&PathBuf> {
        self.archive_path.as_ref()
    }

    /// Get current path within archive
    pub fn get_archive_prefix(&self) -> &str {
        &self.archive_prefix
    }

    pub fn parent_directory(&mut self) -> bool {
        // In archive mode
        if self.archive_path.is_some() {
            if self.archive_prefix.is_empty() {
                // At archive root, exit archive mode
                self.exit_archive();
                return true;
            }
            // Go up within archive
            if let Some(pos) = self.archive_prefix.rfind('/') {
                self.archive_prefix = self.archive_prefix[..pos].to_string();
            } else {
                self.archive_prefix.clear();
            }
            self.refresh();
            return true;
        }

        let Some(parent) = self.path.parent() else { return false };

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

    /// Navigate to a specific path
    pub fn navigate_to(&mut self, path: &PathBuf) {
        if path.is_dir() {
            self.path = path.clone();
            self.refresh();
            self.cursor = 0;
        }
    }

    /// Filter entries by name (case-insensitive substring match)
    pub fn filter_by_name(&mut self, pattern: &str) {
        self.filter = Some(pattern.to_string());
        self.apply_filter();
        // Move cursor to first match if current position is out of bounds
        if self.cursor >= self.entries.len() {
            self.cursor = 0;
        }
    }

    /// Clear the current filter
    pub fn clear_filter(&mut self) {
        self.filter = None;
        self.apply_filter();
    }

    /// Move cursor to next entry matching the search pattern
    pub fn search_next(&mut self, pattern: &str) {
        let pattern_lower = pattern.to_lowercase();
        let start = self.cursor + 1;

        // Search from cursor to end
        for i in start..self.entries.len() {
            if self.entries[i].name.to_lowercase().contains(&pattern_lower) {
                self.cursor = i;
                return;
            }
        }

        // Wrap around: search from start to cursor
        for i in 0..self.cursor {
            if self.entries[i].name.to_lowercase().contains(&pattern_lower) {
                self.cursor = i;
                return;
            }
        }
    }

    /// Move cursor to previous entry matching the search pattern
    pub fn search_prev(&mut self, pattern: &str) {
        let pattern_lower = pattern.to_lowercase();

        // Search backwards from cursor
        if self.cursor > 0 {
            for i in (0..self.cursor).rev() {
                if self.entries[i].name.to_lowercase().contains(&pattern_lower) {
                    self.cursor = i;
                    return;
                }
            }
        }

        // Wrap around: search from end to cursor
        for i in (self.cursor + 1..self.entries.len()).rev() {
            if self.entries[i].name.to_lowercase().contains(&pattern_lower) {
                self.cursor = i;
                return;
            }
        }
    }

    /// Set sort mode and refresh
    pub fn set_sort(&mut self, mode: SortMode, reverse: bool) {
        self.sort_mode = mode;
        self.sort_reverse = reverse;
        self.refresh();
    }
}

pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub is_cut: bool,
    // Archive source for copying files from archives
    archive_source: Option<PathBuf>,
    archive_files: Vec<String>,  // Paths within the archive
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

    /// Yank files from within an archive
    pub fn yank_from_archive(&mut self, archive_path: PathBuf, file_paths: Vec<String>) {
        self.paths.clear();
        self.is_cut = false;
        self.archive_source = Some(archive_path);
        self.archive_files = file_paths;
    }

    /// Check if clipboard contains archive files
    pub fn is_from_archive(&self) -> bool {
        self.archive_source.is_some()
    }

    pub fn paste_to(&mut self, dest_dir: &PathBuf) -> std::io::Result<()> {
        // Handle archive extraction
        if let Some(ref archive_path) = self.archive_source {
            filesystem::extract_files_from_archive(archive_path, &self.archive_files, dest_dir)?;
            // Don't clear archive clipboard (can paste multiple times)
            return Ok(());
        }

        // Normal file copy/move
        for src in &self.paths {
            let Some(name) = src.file_name() else { continue };
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

pub struct Selection {
    indices: Vec<usize>,
}

impl Selection {
    pub fn new() -> Self {
        Self { indices: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.indices.clear();
    }

    pub fn add(&mut self, index: usize) {
        if !self.indices.contains(&index) {
            self.indices.push(index);
        }
    }

    pub fn contains(&self, index: usize) -> bool {
        self.indices.contains(&index)
    }

    pub fn to_paths(&self, entries: &[Entry]) -> Vec<PathBuf> {
        self.indices
            .iter()
            .filter_map(|&i| entries.get(i))
            .map(|e| e.path.clone())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}
