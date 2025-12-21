//! Mode change handlers

use crate::app::{App, CommandResult, FocusArea};
use crate::input::Mode;

impl App {
    pub fn execute_enter_visual_mode(&mut self) -> bool {
        self.mode = Mode::Visual;
        self.selection.clear();
        if let Some(browser) = self.browser() {
            self.selection.add(browser.cursor);
        }
        true
    }

    pub fn execute_exit_visual_mode(&mut self) -> bool {
        if self.feature_pane.visible {
            if self.feature_pane.showing_detail {
                self.feature_pane.showing_detail = false;
            } else {
                self.feature_pane.hide();
                self.focus_area = FocusArea::Splits;
            }
        } else if self.mode == Mode::Visual {
            self.mode = Mode::Normal;
            self.selection.clear();
        } else if self.search_active {
            // Cancel active search (ESC after search was executed)
            self.execute_clear_search_highlight();
        }
        true
    }

    pub fn execute_enter_command_mode(&mut self) -> bool {
        self.mode = Mode::Command;
        self.command_buffer.clear();
        true
    }

    pub fn execute_command_append(&mut self, c: char) -> bool {
        self.command_buffer.push(c);
        true
    }

    pub fn execute_command_backspace(&mut self) -> bool {
        self.command_buffer.pop();
        true
    }

    pub fn execute_command_execute(&mut self) -> bool {
        let result = self.execute_command();
        self.command_buffer.clear();
        self.mode = Mode::Normal;
        match result {
            CommandResult::None => false,
            CommandResult::Redraw => true,
            CommandResult::ThemeChange(name) => {
                self.pending_theme = Some(name);
                true
            }
            CommandResult::Save => {
                if let Err(e) = self.current_settings().save() {
                    eprintln!("failed to save config: {e}");
                }
                true
            }
            CommandResult::Exit => {
                self.should_exit = true;
                false
            }
        }
    }

    pub fn execute_command_cancel(&mut self) -> bool {
        self.command_buffer.clear();
        self.mode = Mode::Normal;
        true
    }
}
