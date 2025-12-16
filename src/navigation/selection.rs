//! Visual mode selection

use std::path::PathBuf;

use crate::filesystem::Entry;

/// Selection state for visual mode
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
