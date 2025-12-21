//! Bookmark and sort action handlers

use crate::app::App;

impl App {
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
}
