//! Split and focus action handlers

use crate::app::{App, FocusArea};
use crate::navigation::Browser;

impl App {
    pub fn execute_focus_left(&mut self) -> bool {
        if self.focus_area == FocusArea::Splits {
            self.splits.focus_left();
        }
        true
    }

    pub fn execute_focus_right(&mut self) -> bool {
        if self.focus_area == FocusArea::Splits {
            self.splits.focus_right();
        }
        true
    }

    pub fn execute_focus_up(&mut self) -> bool {
        if self.focus_area == FocusArea::TaskList {
            self.focus_area = FocusArea::Splits;
        } else {
            self.splits.focus_up();
        }
        true
    }

    pub fn execute_focus_down(&mut self) -> bool {
        let list_visible = self.task_list.visible || self.error_list.visible;
        if self.focus_area == FocusArea::Splits && list_visible {
            self.focus_area = FocusArea::TaskList;
        } else if self.focus_area == FocusArea::Splits {
            self.splits.focus_down();
        }
        true
    }

    pub fn execute_split_vertical(&mut self) -> bool {
        if let Some(browser) = self.browser() {
            let path = browser.path.clone();
            let show_hidden = browser.show_hidden;
            let show_parent = browser.show_parent_entry;
            let new_browser = Browser::new(show_hidden, show_parent, Some(path));
            self.splits.split_vertical(new_browser);
        }
        true
    }

    pub fn execute_split_horizontal(&mut self) -> bool {
        if let Some(browser) = self.browser() {
            let path = browser.path.clone();
            let show_hidden = browser.show_hidden;
            let show_parent = browser.show_parent_entry;
            let new_browser = Browser::new(show_hidden, show_parent, Some(path));
            self.splits.split_horizontal(new_browser);
        }
        true
    }

    pub fn execute_close_split(&mut self) -> bool {
        if self.splits.len() > 1 {
            self.splits.close_focused();
        }
        true
    }
}
