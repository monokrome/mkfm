//! Search action handlers

use crate::app::App;
use crate::input::Mode;

impl App {
    pub fn execute_enter_search_mode(&mut self) -> bool {
        // Save current cursor position before entering search
        self.pre_search_cursor = self.browser().map(|b| b.cursor);
        self.mode = Mode::Search;
        self.search_buffer.clear();
        true
    }

    pub fn execute_search_append(&mut self, c: char) -> bool {
        self.search_buffer.push(c);

        if self.search_narrowing {
            // Old behavior: narrow the list
            self.apply_search_filter();
        } else {
            // Vim-like behavior: just move cursor to first match
            self.move_to_first_incremental_match();
        }
        true
    }

    pub fn execute_search_backspace(&mut self) -> bool {
        self.search_buffer.pop();

        if self.search_narrowing {
            self.apply_search_filter();
        } else if self.search_buffer.is_empty() {
            // Restore to pre-search cursor if buffer is empty
            if let Some(cursor) = self.pre_search_cursor
                && let Some(browser) = self.browser_mut()
            {
                browser.cursor = cursor;
            }
        } else {
            // Re-evaluate match from original cursor position
            self.move_to_first_incremental_match();
        }
        true
    }

    fn move_to_first_incremental_match(&mut self) {
        if self.search_buffer.is_empty() {
            return;
        }

        let pattern_lower = self.search_buffer.to_lowercase();
        let pre_cursor = self.pre_search_cursor.unwrap_or(0);

        // First, search from pre_search_cursor forward
        if let Some(browser) = self.browser() {
            for i in pre_cursor..browser.entries.len() {
                if browser.entries[i].name.to_lowercase().contains(&pattern_lower) {
                    if let Some(b) = self.browser_mut() {
                        b.cursor = i;
                    }
                    return;
                }
            }
        }

        // Then wrap around from beginning to pre_cursor
        if let Some(browser) = self.browser() {
            for i in 0..pre_cursor {
                if browser.entries[i].name.to_lowercase().contains(&pattern_lower) {
                    if let Some(b) = self.browser_mut() {
                        b.cursor = i;
                    }
                    return;
                }
            }
        }
    }

    pub fn execute_search_execute(&mut self) -> bool {
        if self.search_buffer.is_empty() {
            self.clear_search_state();
            self.pre_search_cursor = None;
            self.search_active = false;
        } else {
            // Activate search - keep it active until ESC/CTRL+L
            self.activate_search_after_enter();
            self.search_active = true;
        }

        // If narrowing was not active, the list is already unfiltered
        // If narrowing was active, keep it until ESC/CTRL+L

        self.mode = Mode::Normal;
        true
    }

    fn activate_search_after_enter(&mut self) {
        self.last_search = Some(self.search_buffer.clone());
        self.compute_search_matches();
        self.search_highlight = true;

        // Find the first match AFTER the original cursor position (not current)
        let pre_cursor = self.pre_search_cursor.unwrap_or(0);
        self.current_match = self
            .search_matches
            .iter()
            .position(|&i| i >= pre_cursor)
            .or(if self.search_matches.is_empty() {
                None
            } else {
                Some(0)
            });

        // Move cursor to that match
        if let Some(match_idx) = self.current_match {
            self.move_cursor_to_match(match_idx);
        }
    }

    fn clear_search_state(&mut self) {
        self.last_search = None;
        self.search_highlight = false;
        self.search_matches.clear();
        self.current_match = None;
    }

    pub fn execute_search_cancel(&mut self) -> bool {
        // Restore cursor to pre-search position
        if let Some(cursor) = self.pre_search_cursor
            && let Some(browser) = self.browser_mut()
        {
            browser.cursor = cursor;
        }

        self.search_buffer.clear();
        self.pre_search_cursor = None;
        self.search_active = false;

        if let Some(browser) = self.browser_mut() {
            browser.clear_filter();
        }

        self.mode = Mode::Normal;
        true
    }

    pub fn execute_search_next(&mut self) -> bool {
        if self.search_highlight && !self.search_matches.is_empty() {
            self.move_to_next_match();
        } else if let Some(pattern) = self.last_search.clone()
            && let Some(browser) = self.browser_mut()
        {
            browser.search_next(&pattern);
        }
        true
    }

    fn move_to_next_match(&mut self) {
        let next = self
            .current_match
            .map(|i| (i + 1) % self.search_matches.len())
            .unwrap_or(0);
        self.current_match = Some(next);
        self.move_cursor_to_match(next);
    }

    pub fn execute_search_prev(&mut self) -> bool {
        if self.search_highlight && !self.search_matches.is_empty() {
            self.move_to_prev_match();
        } else if let Some(pattern) = self.last_search.clone()
            && let Some(browser) = self.browser_mut()
        {
            browser.search_prev(&pattern);
        }
        true
    }

    fn move_to_prev_match(&mut self) {
        let len = self.search_matches.len();
        let prev = self
            .current_match
            .map(|i| if i == 0 { len - 1 } else { i - 1 })
            .unwrap_or(len - 1);
        self.current_match = Some(prev);
        self.move_cursor_to_match(prev);
    }

    fn move_cursor_to_match(&mut self, index: usize) {
        let target = self.search_matches[index];
        if let Some(browser) = self.browser_mut() {
            browser.cursor = target;
        }
    }

    pub fn execute_clear_search_highlight(&mut self) -> bool {
        // If search was active (post-Enter), restore cursor and clear filter
        if self.search_active {
            if let Some(cursor) = self.pre_search_cursor
                && let Some(browser) = self.browser_mut()
            {
                browser.cursor = cursor;
            }

            // Clear filter if narrowing was enabled
            if self.search_narrowing
                && let Some(browser) = self.browser_mut()
            {
                browser.clear_filter();
            }
        }

        self.search_highlight = false;
        self.search_matches.clear();
        self.current_match = None;
        self.pre_search_cursor = None;
        self.search_active = false;
        true
    }

    pub fn execute_set_mark(&mut self, c: char) -> bool {
        if let Some(browser) = self.browser() {
            self.bookmarks.insert(c, browser.path.clone());
        }
        false
    }

    pub fn execute_jump_to_mark(&mut self, c: char) -> bool {
        if let Some(path) = self.bookmarks.get(&c).cloned()
            && let Some(browser) = self.browser_mut()
        {
            browser.navigate_to(&path);
        }
        true
    }

    pub fn execute_cycle_sort(&mut self) -> bool {
        self.sort_mode = self.sort_mode.next();
        self.apply_current_sort();
        true
    }

    pub fn execute_reverse_sort(&mut self) -> bool {
        self.sort_reverse = !self.sort_reverse;
        self.apply_current_sort();
        true
    }

    fn apply_current_sort(&mut self) {
        let (mode, reverse) = (self.sort_mode, self.sort_reverse);
        if let Some(browser) = self.browser_mut() {
            browser.set_sort(mode, reverse);
        }
    }

    pub fn execute_clear_filter(&mut self) -> bool {
        self.filter_pattern = None;
        if let Some(browser) = self.browser_mut() {
            browser.clear_filter();
        }
        true
    }

    pub fn apply_search_filter(&mut self) {
        let pattern = if self.search_buffer.is_empty() {
            None
        } else {
            Some(self.search_buffer.clone())
        };

        if let Some(browser) = self.browser_mut() {
            match pattern {
                Some(p) => browser.filter_by_name(&p),
                None => browser.clear_filter(),
            }
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
