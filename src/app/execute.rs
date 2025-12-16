//! Action execution dispatch

use crate::input::Action;

use super::{App, CommandResult};

impl App {
    /// Execute an action, returns true if redraw needed
    pub fn execute(&mut self, action: Action) -> bool {
        match action {
            Action::None | Action::Pending => false,

            // Navigation actions
            Action::MoveCursor(delta) => self.execute_move_cursor(delta),
            Action::CursorToTop => self.execute_cursor_to_top(),
            Action::CursorToBottom => self.execute_cursor_to_bottom(),
            Action::NextDirectory => self.execute_next_directory(),
            Action::PrevDirectory => self.execute_prev_directory(),
            Action::EnterDirectory => self.execute_enter_directory(),
            Action::ParentDirectory => self.execute_parent_directory(),

            // File operations
            Action::OpenFile => self.execute_open_file(),
            Action::Yank => self.execute_yank(),
            Action::Cut => self.execute_cut(),
            Action::Paste => self.execute_paste(),
            Action::Delete => self.execute_delete(),
            Action::Trash => self.execute_trash(),
            Action::CreateSymlink => self.execute_create_symlink(),
            Action::ExtractArchive => self.execute_extract_archive(),

            // Mode changes
            Action::EnterVisualMode => self.execute_enter_visual_mode(),
            Action::ExitVisualMode => self.execute_exit_visual_mode(),
            Action::EnterCommandMode => self.execute_enter_command_mode(),
            Action::CommandAppend(c) => self.execute_command_append(c),
            Action::CommandBackspace => self.execute_command_backspace(),
            Action::CommandExecute => self.execute_command_execute(),
            Action::CommandCancel => self.execute_command_cancel(),

            // Toggle actions
            Action::ToggleHidden => self.execute_toggle_hidden(),
            Action::EnableHidden => self.execute_enable_hidden(),
            Action::DisableHidden => self.execute_disable_hidden(),
            Action::ToggleOverlay => self.execute_toggle_overlay(),
            Action::EnableOverlay => self.execute_enable_overlay(),
            Action::DisableOverlay => self.execute_disable_overlay(),

            // Split/focus actions
            Action::FocusLeft => self.execute_focus_left(),
            Action::FocusRight => self.execute_focus_right(),
            Action::FocusUp => self.execute_focus_up(),
            Action::FocusDown => self.execute_focus_down(),
            Action::SplitVertical => self.execute_split_vertical(),
            Action::SplitHorizontal => self.execute_split_horizontal(),
            Action::CloseSplit => self.execute_close_split(),

            // Search actions
            Action::EnterSearchMode => self.execute_enter_search_mode(),
            Action::SearchAppend(c) => self.execute_search_append(c),
            Action::SearchBackspace => self.execute_search_backspace(),
            Action::SearchExecute => self.execute_search_execute(),
            Action::SearchCancel => self.execute_search_cancel(),
            Action::SearchNext => self.execute_search_next(),
            Action::SearchPrev => self.execute_search_prev(),
            Action::ClearSearchHighlight => self.execute_clear_search_highlight(),

            // Bookmark actions
            Action::SetMark(c) => self.execute_set_mark(c),
            Action::JumpToMark(c) => self.execute_jump_to_mark(c),

            // Sort/filter actions
            Action::CycleSort => self.execute_cycle_sort(),
            Action::ReverseSort => self.execute_reverse_sort(),
            Action::ClearFilter => self.execute_clear_filter(),

            // Fold actions
            Action::FoldOpen => self.execute_fold_open(),
            Action::FoldClose => self.execute_fold_close(),
            Action::FoldToggle => self.execute_fold_toggle(),
            Action::FoldOpenRecursive => self.execute_fold_open_recursive(),
            Action::FoldCloseRecursive => self.execute_fold_close_recursive(),

            // Task/error list actions
            Action::NextTask => self.execute_next_task(),
            Action::PrevTask => self.execute_prev_task(),
            Action::ToggleTaskList => self.execute_toggle_task_list(),
            Action::NextError => self.execute_next_error(),
            Action::PrevError => self.execute_prev_error(),
            Action::ToggleErrorList => self.execute_toggle_error_list(),
            Action::ToggleFeatureList => self.execute_toggle_feature_list(),
        }
    }

    /// Execute a command from command mode
    pub fn execute_command(&mut self) -> CommandResult {
        let cmd = self.command_buffer.trim().to_string();

        if let Some(rest) = cmd.strip_prefix("set ").or_else(|| cmd.strip_prefix("se ")) {
            return self.execute_set_command(rest.trim());
        }

        if let Some(pattern) = cmd
            .strip_prefix("filter ")
            .or_else(|| cmd.strip_prefix("f "))
        {
            return self.execute_filter_command(pattern.trim());
        }

        if let Some(mode) = cmd.strip_prefix("sort ") {
            return self.execute_sort_command(mode.trim());
        }

        if let Some(mode) = cmd.strip_prefix("chmod ") {
            return self.execute_chmod_command(mode.trim());
        }

        if cmd == "rename" || cmd == "bulkrename" {
            self.execute_bulk_rename();
            return CommandResult::Redraw;
        }

        self.execute_simple_command(&cmd)
    }

    fn execute_set_command(&mut self, arg: &str) -> CommandResult {
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            if key == "theme" {
                let theme_name = if value.is_empty() || value == "default" {
                    None
                } else {
                    Some(value.to_string())
                };
                return CommandResult::ThemeChange(theme_name);
            }
            return CommandResult::Redraw;
        }

        let (negated, option) = if let Some(opt) = arg.strip_prefix("no") {
            (true, opt)
        } else {
            (false, arg)
        };

        match option {
            "hidden" | "hid" => {
                if let Some(browser) = self.browser_mut() {
                    if negated == browser.show_hidden {
                        browser.toggle_hidden();
                    }
                }
                CommandResult::Redraw
            }
            "overlay" | "ol" => {
                self.overlay_enabled = !negated;
                CommandResult::Redraw
            }
            "parent" | "par" => {
                if let Some(browser) = self.browser_mut() {
                    browser.show_parent_entry = !negated;
                    browser.refresh();
                }
                CommandResult::Redraw
            }
            _ => CommandResult::Redraw,
        }
    }

    fn execute_filter_command(&mut self, pattern: &str) -> CommandResult {
        let filter = if pattern.is_empty() {
            None
        } else {
            Some(pattern.to_string())
        };
        self.filter_pattern = filter.clone();
        if let Some(browser) = self.browser_mut() {
            match filter {
                Some(p) => browser.filter_by_name(&p),
                None => browser.clear_filter(),
            }
        }
        CommandResult::Redraw
    }

    fn execute_sort_command(&mut self, mode: &str) -> CommandResult {
        use crate::input::SortMode;
        self.sort_mode = match mode {
            "name" | "n" => SortMode::Name,
            "size" | "s" => SortMode::Size,
            "date" | "d" | "time" | "t" => SortMode::Date,
            "type" | "ext" | "e" => SortMode::Type,
            _ => self.sort_mode,
        };
        let (sm, sr) = (self.sort_mode, self.sort_reverse);
        if let Some(browser) = self.browser_mut() {
            browser.set_sort(sm, sr);
        }
        CommandResult::Redraw
    }

    fn execute_chmod_command(&mut self, mode: &str) -> CommandResult {
        if let Some(browser) = self.browser()
            && let Some(entry) = browser.current_entry()
        {
            let _ = crate::filesystem::chmod(&entry.path, mode);
        }
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
        CommandResult::Redraw
    }

    fn execute_simple_command(&mut self, cmd: &str) -> CommandResult {
        use crate::navigation::Browser;

        match cmd {
            "q" | "quit" => {
                if self.splits.len() <= 1 {
                    return CommandResult::Exit;
                }
                self.splits.close_focused();
                CommandResult::Redraw
            }
            "qa" | "qall" | "qa!" | "qall!" => CommandResult::Exit,
            "sp" | "split" => {
                if let Some(browser) = self.browser() {
                    let path = browser.path.clone();
                    let show_hidden = browser.show_hidden;
                    let show_parent = browser.show_parent_entry;
                    let new_browser = Browser::new(show_hidden, show_parent, Some(path));
                    self.splits.split_horizontal(new_browser);
                }
                CommandResult::Redraw
            }
            "vs" | "vsp" | "vsplit" => {
                if let Some(browser) = self.browser() {
                    let path = browser.path.clone();
                    let show_hidden = browser.show_hidden;
                    let show_parent = browser.show_parent_entry;
                    let new_browser = Browser::new(show_hidden, show_parent, Some(path));
                    self.splits.split_vertical(new_browser);
                }
                CommandResult::Redraw
            }
            "w" | "write" => CommandResult::Save,
            "wq" | "x" => {
                if let Err(e) = self.current_settings().save() {
                    eprintln!("failed to save config: {e}");
                }
                CommandResult::Exit
            }
            "filter" | "f" => {
                self.filter_pattern = None;
                if let Some(browser) = self.browser_mut() {
                    browser.clear_filter();
                }
                CommandResult::Redraw
            }
            "ln" | "symlink" => {
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
            _ => CommandResult::Redraw,
        }
    }

    fn execute_bulk_rename(&mut self) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return;
        }

        let temp_path = std::env::temp_dir().join("mkfm_rename.txt");
        let names: Vec<String> = paths
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        if std::fs::write(&temp_path, names.join("\n")).is_err() {
            return;
        }

        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let status = std::process::Command::new(&editor).arg(&temp_path).status();

        if status.is_ok() {
            if let Ok(content) = std::fs::read_to_string(&temp_path) {
                let new_names: Vec<&str> = content.lines().collect();
                for (old_path, new_name) in paths.iter().zip(new_names.iter()) {
                    if let Some(parent) = old_path.parent() {
                        let new_path = parent.join(new_name.trim());
                        if *old_path != new_path && !new_name.trim().is_empty() {
                            let _ = std::fs::rename(old_path, &new_path);
                        }
                    }
                }
            }
        }

        let _ = std::fs::remove_file(&temp_path);
        self.exit_visual_if_active();
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
    }
}
