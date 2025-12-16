//! Navigation action handlers

use crate::app::{App, FocusArea};
use crate::input::Mode;

impl App {
    pub fn execute_move_cursor(&mut self, delta: i32) -> bool {
        if self.focus_area == FocusArea::FeatureList {
            let feature_count = self.feature_list.features.len();
            self.feature_pane.move_cursor(delta, feature_count);
        } else if self.focus_area == FocusArea::TaskList {
            let job_count = self.job_queue.all_jobs().len();
            if job_count > 0 {
                let cursor_ref = if self.error_list.visible && !self.task_list.visible {
                    &mut self.error_list.cursor
                } else {
                    &mut self.task_list.cursor
                };

                if delta > 0 {
                    *cursor_ref = (*cursor_ref + delta as usize).min(job_count - 1);
                } else if delta < 0 {
                    let abs_delta = (-delta) as usize;
                    *cursor_ref = cursor_ref.saturating_sub(abs_delta);
                }
            }
        } else {
            let cursor = if let Some(browser) = self.browser_mut() {
                browser.move_cursor(delta);
                Some(browser.cursor)
            } else {
                None
            };
            if self.mode == Mode::Visual
                && let Some(c) = cursor
            {
                self.selection.add(c);
            }
        }
        true
    }

    pub fn execute_cursor_to_top(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            browser.cursor_to_top();
        }
        true
    }

    pub fn execute_cursor_to_bottom(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            browser.cursor_to_bottom();
        }
        true
    }

    pub fn execute_next_directory(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            browser.next_directory();
        }
        true
    }

    pub fn execute_prev_directory(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            browser.prev_directory();
        }
        true
    }

    pub fn execute_enter_directory(&mut self) -> bool {
        if self.focus_area == FocusArea::FeatureList {
            self.feature_pane.toggle_detail();
        } else if let Some(browser) = self.browser_mut() {
            browser.enter_directory();
        }
        true
    }

    pub fn execute_parent_directory(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            browser.parent_directory();
        }
        true
    }
}
