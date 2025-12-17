//! Search action handlers

use crate::app::App;
use crate::input::Mode;

impl App {
    pub fn execute_enter_search_mode(&mut self) -> bool {
        self.mode = Mode::Search;
        self.search_buffer.clear();
        true
    }

    pub fn execute_search_append(&mut self, c: char) -> bool {
        self.search_buffer.push(c);
        self.apply_search_filter();
        true
    }

    pub fn execute_search_backspace(&mut self) -> bool {
        self.search_buffer.pop();
        self.apply_search_filter();
        true
    }

    pub fn execute_search_execute(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            browser.clear_filter();
        }

        if self.search_buffer.is_empty() {
            self.clear_search_state();
        } else {
            self.activate_search();
        }
        self.mode = Mode::Normal;
        true
    }

    fn clear_search_state(&mut self) {
        self.last_search = None;
        self.search_highlight = false;
        self.search_matches.clear();
        self.current_match = None;
    }

    fn activate_search(&mut self) {
        self.last_search = Some(self.search_buffer.clone());
        self.compute_search_matches();
        self.search_highlight = true;
        self.current_match = self.find_first_match();
    }

    fn find_first_match(&self) -> Option<usize> {
        let cursor = self.browser().map(|b| b.cursor).unwrap_or(0);
        self.search_matches
            .iter()
            .position(|&i| i >= cursor)
            .or(if self.search_matches.is_empty() { None } else { Some(0) })
    }

    pub fn execute_search_cancel(&mut self) -> bool {
        self.search_buffer.clear();
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
            && let Some(browser) = self.browser_mut() {
                browser.search_next(&pattern);
            }
        true
    }

    fn move_to_next_match(&mut self) {
        let next = self.current_match.map(|i| (i + 1) % self.search_matches.len()).unwrap_or(0);
        self.current_match = Some(next);
        self.move_cursor_to_match(next);
    }

    pub fn execute_search_prev(&mut self) -> bool {
        if self.search_highlight && !self.search_matches.is_empty() {
            self.move_to_prev_match();
        } else if let Some(pattern) = self.last_search.clone()
            && let Some(browser) = self.browser_mut() {
                browser.search_prev(&pattern);
            }
        true
    }

    fn move_to_prev_match(&mut self) {
        let len = self.search_matches.len();
        let prev = self.current_match.map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(len - 1);
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
        self.search_highlight = false;
        self.search_matches.clear();
        self.current_match = None;
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
            && let Some(browser) = self.browser_mut() {
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
