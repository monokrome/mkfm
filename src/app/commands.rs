//! Command mode command implementations

use super::{App, CommandResult};
use crate::navigation::Browser;

impl App {
    pub fn execute_set_command(&mut self, arg: &str) -> CommandResult {
        if let Some(result) = self.try_set_theme(arg) {
            return result;
        }

        let (negated, option) = parse_set_option(arg);
        self.apply_set_option(option, negated)
    }

    fn try_set_theme(&mut self, arg: &str) -> Option<CommandResult> {
        let (key, value) = arg.split_once('=')?;
        if key.trim() != "theme" {
            return None;
        }
        let value = value.trim();
        let theme_name = if value.is_empty() || value == "default" {
            None
        } else {
            Some(value.to_string())
        };
        Some(CommandResult::ThemeChange(theme_name))
    }

    fn apply_set_option(&mut self, option: &str, negated: bool) -> CommandResult {
        match option {
            "hidden" | "hid" => self.apply_hidden_option(negated),
            "overlay" | "ol" => {
                self.overlay_enabled = !negated;
                CommandResult::Redraw
            }
            "parent" | "par" => self.apply_parent_option(negated),
            _ => CommandResult::Redraw,
        }
    }

    fn apply_hidden_option(&mut self, negated: bool) -> CommandResult {
        if let Some(browser) = self.browser_mut()
            && negated == browser.show_hidden {
                browser.toggle_hidden();
            }
        CommandResult::Redraw
    }

    fn apply_parent_option(&mut self, negated: bool) -> CommandResult {
        if let Some(browser) = self.browser_mut() {
            browser.show_parent_entry = !negated;
            browser.refresh();
        }
        CommandResult::Redraw
    }

    pub fn execute_simple_command(&mut self, cmd: &str) -> CommandResult {
        match cmd {
            "q" | "quit" => self.cmd_quit(),
            "qa" | "qall" | "qa!" | "qall!" => CommandResult::Exit,
            "sp" | "split" => self.cmd_split_horizontal(),
            "vs" | "vsp" | "vsplit" => self.cmd_split_vertical(),
            "w" | "write" => CommandResult::Save,
            "wq" | "x" => self.cmd_write_quit(),
            "filter" | "f" => self.cmd_clear_filter(),
            "ln" | "symlink" => self.cmd_symlink(),
            _ => CommandResult::Redraw,
        }
    }

    fn cmd_quit(&mut self) -> CommandResult {
        if self.splits.len() <= 1 {
            return CommandResult::Exit;
        }
        self.splits.close_focused();
        CommandResult::Redraw
    }

    fn cmd_split_horizontal(&mut self) -> CommandResult {
        if let Some(browser) = self.browser() {
            let new_browser = Browser::new(
                browser.show_hidden,
                browser.show_parent_entry,
                Some(browser.path.clone()),
            );
            self.splits.split_horizontal(new_browser);
        }
        CommandResult::Redraw
    }

    fn cmd_split_vertical(&mut self) -> CommandResult {
        if let Some(browser) = self.browser() {
            let new_browser = Browser::new(
                browser.show_hidden,
                browser.show_parent_entry,
                Some(browser.path.clone()),
            );
            self.splits.split_vertical(new_browser);
        }
        CommandResult::Redraw
    }

    fn cmd_write_quit(&mut self) -> CommandResult {
        if let Err(e) = self.current_settings().save() {
            eprintln!("failed to save config: {e}");
        }
        CommandResult::Exit
    }

    fn cmd_clear_filter(&mut self) -> CommandResult {
        self.filter_pattern = None;
        if let Some(browser) = self.browser_mut() {
            browser.clear_filter();
        }
        CommandResult::Redraw
    }

    fn cmd_symlink(&mut self) -> CommandResult {
        if let Some(browser) = self.browser() {
            let dest_dir = browser.path.clone();
            for src in &self.clipboard.paths {
                let _ = crate::filesystem::create_symlink(src, &dest_dir);
            }
        }
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
        CommandResult::Redraw
    }

}

fn parse_set_option(arg: &str) -> (bool, &str) {
    if let Some(opt) = arg.strip_prefix("no") {
        (true, opt)
    } else {
        (false, arg)
    }
}
