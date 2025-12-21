//! Search mode handlers

use crate::app::App;
use crate::input::Mode;

impl App {
    pub fn execute_enter_search_mode(&mut self) -> bool {
        self.pre_search_cursor = self.browser().map(|b| b.cursor);
        self.mode = Mode::Search;
        self.search_buffer.clear();
        true
    }

    pub fn execute_search_append(&mut self, c: char) -> bool {
        self.search_buffer.push(c);

        if self.search_narrowing {
            self.apply_search_filter();
        } else {
            self.move_to_first_incremental_match();
        }
        true
    }

    pub fn execute_search_backspace(&mut self) -> bool {
        self.search_buffer.pop();

        if self.search_narrowing {
            self.apply_search_filter();
        } else if self.search_buffer.is_empty() {
            self.restore_pre_search_cursor();
        } else {
            self.move_to_first_incremental_match();
        }
        true
    }

    fn restore_pre_search_cursor(&mut self) {
        if let Some(cursor) = self.pre_search_cursor
            && let Some(browser) = self.browser_mut()
        {
            browser.cursor = cursor;
        }
    }

    pub fn execute_search_execute(&mut self) -> bool {
        if self.search_buffer.is_empty() {
            self.clear_search_state();
            self.pre_search_cursor = None;
            self.search_active = false;
        } else {
            self.activate_search_after_enter();
            self.search_active = true;
        }

        self.mode = Mode::Normal;
        true
    }

    fn activate_search_after_enter(&mut self) {
        self.last_search = Some(self.search_buffer.clone());
        self.compute_search_matches();
        self.search_highlight = true;
        self.current_match = self.find_first_match_after_cursor();

        if let Some(match_idx) = self.current_match {
            self.move_cursor_to_match(match_idx);
        }
    }

    fn find_first_match_after_cursor(&self) -> Option<usize> {
        let pre_cursor = self.pre_search_cursor.unwrap_or(0);
        self.search_matches
            .iter()
            .position(|&i| i >= pre_cursor)
            .or(if self.search_matches.is_empty() {
                None
            } else {
                Some(0)
            })
    }

    fn clear_search_state(&mut self) {
        self.last_search = None;
        self.search_highlight = false;
        self.search_matches.clear();
        self.current_match = None;
    }

    pub fn execute_search_cancel(&mut self) -> bool {
        self.restore_pre_search_cursor();
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

    pub fn execute_clear_search_highlight(&mut self) -> bool {
        if self.search_active {
            self.restore_pre_search_cursor();

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
}
