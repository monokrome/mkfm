use crate::filesystem::{self, ArchiveEntry, Entry};
use crate::input::SortMode;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct Browser {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    all_entries: Vec<Entry>, // Unfiltered entries for search
    pub cursor: usize,
    pub show_hidden: bool,
    pub show_parent_entry: bool,
    sort_mode: SortMode,
    sort_reverse: bool,
    filter: Option<String>,
    // Archive browsing state
    archive_path: Option<PathBuf>, // The archive file being browsed
    archive_prefix: String,        // Current path within the archive
    archive_entries: Vec<ArchiveEntry>, // All entries in the archive
    // Inline expansion (fold) state
    expanded_dirs: HashSet<PathBuf>, // Directories that are expanded inline
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

        // Apply hidden filter
        if !self.show_hidden {
            self.all_entries
                .retain(|e| !e.name.starts_with('.') || (self.show_parent_entry && e.name == ".."));
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
                depth: 0,
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
                entry_path
                    .strip_prefix(prefix)
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
                        depth: 0,
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
                        depth: 0,
                    });
                }
            } else {
                self.all_entries.push(Entry {
                    name: name.to_string(),
                    path: PathBuf::from(&entry.full_path),
                    is_dir: false,
                    size: entry.size,
                    modified: None,
                    depth: 0,
                });
            }
        }

        // Apply sorting
        Self::sort_entries_impl(&mut self.all_entries, self.sort_mode, self.sort_reverse);

        // Apply name filter if active
        self.apply_filter();

        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    fn sort_entries_impl(entries: &mut [Entry], sort_mode: SortMode, sort_reverse: bool) {
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
            SortMode::Size => slice.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.size.cmp(&b.size),
            }),
            SortMode::Date => slice.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.modified.cmp(&b.modified),
            }),
            SortMode::Type => slice.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    let ext_a = a.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let ext_b = b.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
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
        let Some(entry) = self.current_entry().cloned() else {
            return false;
        };

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

        if !entry.is_dir {
            return false;
        }

        self.path = entry.path.clone();
        self.refresh();
        self.cursor = 0;
        true
    }

    /// Enter archive browsing mode
    fn enter_archive(&mut self, archive_path: &Path) {
        self.archive_entries = filesystem::list_archive(archive_path);
        self.archive_path = Some(archive_path.to_path_buf());
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
    pub fn get_archive_path(&self) -> Option<&Path> {
        self.archive_path.as_deref()
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

    /// Navigate to a specific path
    pub fn navigate_to(&mut self, path: &Path) {
        if path.is_dir() {
            self.path = path.to_path_buf();
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

    // ==================== Fold (inline expansion) methods ====================

    /// Check if a directory is expanded
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_dirs.contains(path)
    }

    /// Expand a directory inline at the given index
    pub fn expand_directory(&mut self, index: usize, recursive: bool) {
        let Some(entry) = self.entries.get(index).cloned() else {
            return;
        };
        if !entry.is_dir || entry.name == ".." {
            return;
        }
        if self.expanded_dirs.contains(&entry.path) {
            return;
        }

        self.expanded_dirs.insert(entry.path.clone());

        // Load children entries
        let children = self.load_children(&entry.path, entry.depth + 1);

        // Insert children after the parent entry
        let insert_pos = index + 1;
        for (i, child) in children.into_iter().enumerate() {
            if recursive && child.is_dir && child.name != ".." {
                // Mark for recursive expansion (will be expanded in next pass)
                self.expanded_dirs.insert(child.path.clone());
            }
            self.entries.insert(insert_pos + i, child);
        }

        // If recursive, expand all newly added directories
        if recursive {
            self.rebuild_with_expansions();
        }
    }

    /// Collapse a directory at the given index
    pub fn collapse_directory(&mut self, index: usize, recursive: bool) {
        let Some(entry) = self.entries.get(index).cloned() else {
            return;
        };
        if !entry.is_dir || entry.name == ".." {
            return;
        }
        if !self.expanded_dirs.contains(&entry.path) {
            return;
        }

        // Find range of children to remove
        let range = self.find_children_range(index);

        if recursive {
            // Also remove from expanded_dirs any nested directories
            for i in range.clone() {
                if let Some(child) = self.entries.get(i)
                    && child.is_dir
                {
                    self.expanded_dirs.remove(&child.path);
                }
            }
        }

        self.expanded_dirs.remove(&entry.path);

        // Remove children from entries
        if !range.is_empty() {
            self.entries.drain(range);
        }

        // Adjust cursor if needed
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    /// Toggle expansion state of directory at index
    pub fn toggle_expansion(&mut self, index: usize, recursive: bool) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        if !entry.is_dir || entry.name == ".." {
            return;
        }

        if self.expanded_dirs.contains(&entry.path) {
            self.collapse_directory(index, recursive);
        } else {
            self.expand_directory(index, recursive);
        }
    }

    /// Find the range of children entries for a directory at index
    fn find_children_range(&self, index: usize) -> std::ops::Range<usize> {
        let parent_depth = self.entries.get(index).map(|e| e.depth).unwrap_or(0);
        let start = index + 1;

        let end = self.entries[start..]
            .iter()
            .position(|e| e.depth <= parent_depth)
            .map(|p| start + p)
            .unwrap_or(self.entries.len());

        start..end
    }

    /// Load children of a directory with specified depth
    fn load_children(&self, dir_path: &Path, depth: u8) -> Vec<Entry> {
        let mut children = filesystem::list_directory(dir_path);

        // Remove ".." entry and set depth
        children.retain(|e| e.name != "..");
        for entry in &mut children {
            entry.depth = depth;
        }

        // Apply hidden filter
        if !self.show_hidden {
            children.retain(|e| !e.name.starts_with('.'));
        }

        // Apply sorting
        Self::sort_entries_impl(&mut children, self.sort_mode, self.sort_reverse);

        children
    }

    /// Rebuild entries respecting current expansion state
    fn rebuild_with_expansions(&mut self) {
        // Start with base directory entries
        let mut new_entries = filesystem::list_directory(&self.path);

        // Apply hidden filter
        if !self.show_hidden {
            new_entries
                .retain(|e| !e.name.starts_with('.') || (self.show_parent_entry && e.name == ".."));
        }

        // Apply sorting
        Self::sort_entries_impl(&mut new_entries, self.sort_mode, self.sort_reverse);

        // Recursively insert expanded directories
        let mut i = 0;
        while i < new_entries.len() {
            let entry = &new_entries[i];
            if entry.is_dir && entry.name != ".." && self.expanded_dirs.contains(&entry.path) {
                let children = self.load_children(&entry.path, entry.depth + 1);
                let insert_pos = i + 1;
                for (j, child) in children.into_iter().enumerate() {
                    new_entries.insert(insert_pos + j, child);
                }
            }
            i += 1;
        }

        self.entries = new_entries.clone();
        self.all_entries = new_entries;
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }
}

pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub is_cut: bool,
    // Archive source for copying files from archives
    archive_source: Option<PathBuf>,
    archive_files: Vec<String>, // Paths within the archive
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

    pub fn paste_to(&mut self, dest_dir: &Path) -> std::io::Result<()> {
        // Handle archive extraction
        if let Some(ref archive_path) = self.archive_source {
            filesystem::extract_files_from_archive(archive_path, &self.archive_files, dest_dir)?;
            // Don't clear archive clipboard (can paste multiple times)
            return Ok(());
        }

        // Normal file copy/move
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

pub struct Selection {
    indices: Vec<usize>,
}

impl Selection {
    pub fn new() -> Self {
        Self {
            indices: Vec::new(),
        }
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

    pub fn remove(&mut self, index: usize) {
        self.indices.retain(|&i| i != index);
    }

    pub fn toggle(&mut self, index: usize) {
        if self.contains(index) {
            self.remove(index);
        } else {
            self.add(index);
        }
    }
}
