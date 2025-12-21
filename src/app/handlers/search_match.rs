//! Search match finding and navigation

use crate::app::App;

impl App {
    pub(super) fn move_to_first_incremental_match(&mut self) {
        if self.search_buffer.is_empty() {
            return;
        }

        let pattern_lower = self.search_buffer.to_lowercase();
        let pre_cursor = self.pre_search_cursor.unwrap_or(0);

        if self.find_match_forward(&pattern_lower, pre_cursor) {
            return;
        }
        self.find_match_wrap(&pattern_lower, pre_cursor);
    }

    fn find_match_forward(&mut self, pattern: &str, start: usize) -> bool {
        let Some(browser) = self.browser() else {
            return false;
        };
        for i in start..browser.entries.len() {
            if browser.entries[i].name.to_lowercase().contains(pattern) {
                if let Some(b) = self.browser_mut() {
                    b.cursor = i;
                }
                return true;
            }
        }
        false
    }

    fn find_match_wrap(&mut self, pattern: &str, end: usize) -> bool {
        let Some(browser) = self.browser() else {
            return false;
        };
        for i in 0..end {
            if browser.entries[i].name.to_lowercase().contains(pattern) {
                if let Some(b) = self.browser_mut() {
                    b.cursor = i;
                }
                return true;
            }
        }
        false
    }

    pub(super) fn move_to_next_match(&mut self) {
        let next = self
            .current_match
            .map(|i| (i + 1) % self.search_matches.len())
            .unwrap_or(0);
        self.current_match = Some(next);
        self.move_cursor_to_match(next);
    }

    pub(super) fn move_to_prev_match(&mut self) {
        let len = self.search_matches.len();
        let prev = self
            .current_match
            .map(|i| if i == 0 { len - 1 } else { i - 1 })
            .unwrap_or(len - 1);
        self.current_match = Some(prev);
        self.move_cursor_to_match(prev);
    }

    pub(super) fn move_cursor_to_match(&mut self, index: usize) {
        let target = self.search_matches[index];
        if let Some(browser) = self.browser_mut() {
            browser.cursor = target;
        }
    }

    pub fn compute_search_matches(&mut self) {
        self.search_matches.clear();
        let Some(ref pattern) = self.last_search else {
            return;
        };
        let pattern_lower = pattern.to_lowercase();
        if let Some(browser) = self.browser() {
            self.search_matches = browser
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.name.to_lowercase().contains(&pattern_lower))
                .map(|(i, _)| i)
                .collect();
        }
    }
}
