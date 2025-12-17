//! Filter and search methods for the browser

use super::Browser;

impl Browser {
    // Filter
    pub fn filter_by_name(&mut self, pattern: &str) {
        self.filter = Some(pattern.to_string());
        self.apply_filter();
        if self.cursor >= self.entries.len() {
            self.cursor = 0;
        }
    }

    pub fn clear_filter(&mut self) {
        self.filter = None;
        self.apply_filter();
    }

    // Search
    pub fn search_next(&mut self, pattern: &str) {
        let pattern_lower = pattern.to_lowercase();
        let start = self.cursor + 1;

        for i in start..self.entries.len() {
            if self.entries[i].name.to_lowercase().contains(&pattern_lower) {
                self.cursor = i;
                return;
            }
        }

        for i in 0..self.cursor {
            if self.entries[i].name.to_lowercase().contains(&pattern_lower) {
                self.cursor = i;
                return;
            }
        }
    }

    pub fn search_prev(&mut self, pattern: &str) {
        let pattern_lower = pattern.to_lowercase();

        if self.cursor > 0 {
            for i in (0..self.cursor).rev() {
                if self.entries[i].name.to_lowercase().contains(&pattern_lower) {
                    self.cursor = i;
                    return;
                }
            }
        }

        for i in (self.cursor + 1..self.entries.len()).rev() {
            if self.entries[i].name.to_lowercase().contains(&pattern_lower) {
                self.cursor = i;
                return;
            }
        }
    }
}
