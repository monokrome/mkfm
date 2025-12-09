use std::path::PathBuf;
use crate::filesystem::{self, Entry};

pub struct Browser {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub cursor: usize,
    pub show_hidden: bool,
    pub show_parent_entry: bool,
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
            cursor: 0,
            show_hidden,
            show_parent_entry,
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
        self.entries = filesystem::list_directory(&self.path);

        if !self.show_hidden {
            self.entries.retain(|e| !e.name.starts_with('.') || (self.show_parent_entry && e.name == ".."));
        }

        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
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
        let Some(entry) = self.current_entry() else { return false };
        if !entry.is_dir { return false; }

        self.path = entry.path.clone();
        self.refresh();
        self.cursor = 0;
        true
    }

    pub fn parent_directory(&mut self) -> bool {
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
}

pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub is_cut: bool,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            is_cut: false,
        }
    }

    pub fn yank(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.is_cut = false;
    }

    pub fn cut(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.is_cut = true;
    }

    pub fn paste_to(&mut self, dest_dir: &PathBuf) -> std::io::Result<()> {
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
