//! Fold (directory expansion) action handlers

use crate::app::App;

impl App {
    pub fn execute_fold_open(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            let cursor = browser.cursor;
            browser.expand_directory(cursor, false);
        }
        true
    }

    pub fn execute_fold_close(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            let cursor = browser.cursor;
            browser.collapse_directory(cursor, false);
        }
        true
    }

    pub fn execute_fold_toggle(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            let cursor = browser.cursor;
            browser.toggle_expansion(cursor, false);
        }
        true
    }

    pub fn execute_fold_open_recursive(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            let cursor = browser.cursor;
            browser.expand_directory(cursor, true);
        }
        true
    }

    pub fn execute_fold_close_recursive(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            let cursor = browser.cursor;
            browser.collapse_directory(cursor, true);
        }
        true
    }
}
