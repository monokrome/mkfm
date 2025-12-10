mod config;
mod filesystem;
mod input;
mod navigation;

use std::path::PathBuf;

use config::{Config, Openers, OverlayPosition, SavedSettings, Theme};
use libc;
use image::GenericImageView;

enum PreviewContent {
    Image { data: Vec<u8>, width: u32, height: u32 },
    Text(Vec<String>),
    Unsupported(String),
    Error(String),
}

impl PreviewContent {
    fn dimensions(&self, max_width: u32, max_height: u32) -> (u32, u32) {
        match self {
            PreviewContent::Image { width, height, .. } => (*width, *height),
            PreviewContent::Text(lines) => {
                // Estimate text dimensions
                let line_height = 16u32;
                let char_width = 8u32;
                let max_line_len = lines.iter().map(|l| l.len().min(80)).max().unwrap_or(20) as u32;
                let height = (lines.len() as u32 * line_height).min(max_height);
                let width = (max_line_len * char_width).min(max_width);
                (width.max(100), height.max(50))
            }
            PreviewContent::Unsupported(_) | PreviewContent::Error(_) => {
                // Small fixed size for messages
                (200, 50)
            }
        }
    }
}

fn load_preview_content(path: &std::path::Path, max_width: u32, max_height: u32) -> PreviewContent {
    if is_image_file(path) {
        match image::open(path) {
            Ok(img) => {
                let (img_w, img_h) = img.dimensions();

                // Sanity check dimensions
                if img_w == 0 || img_h == 0 {
                    return PreviewContent::Error("Invalid image dimensions".to_string());
                }

                // Calculate target size maintaining aspect ratio
                let scale_x = max_width as f32 / img_w as f32;
                let scale_y = max_height as f32 / img_h as f32;
                let scale = scale_x.min(scale_y).min(1.0);

                let target_w = ((img_w as f32 * scale) as u32).max(1);
                let target_h = ((img_h as f32 * scale) as u32).max(1);

                // Resize if needed
                let rgba = if target_w < img_w || target_h < img_h {
                    img.resize_exact(target_w, target_h, image::imageops::FilterType::Triangle)
                        .to_rgba8()
                } else {
                    img.to_rgba8()
                };

                let (final_w, final_h) = rgba.dimensions();
                PreviewContent::Image {
                    data: rgba.into_raw(),
                    width: final_w,
                    height: final_h,
                }
            }
            Err(e) => PreviewContent::Error(format!("Load failed: {}", e)),
        }
    } else if is_text_file(path) {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let lines: Vec<String> = content.lines().take(50).map(|s| s.to_string()).collect();
                PreviewContent::Text(lines)
            }
            Err(e) => PreviewContent::Error(format!("Read failed: {}", e)),
        }
    } else {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
        PreviewContent::Unsupported(filename.to_string())
    }
}

struct PreviewCache {
    path: Option<PathBuf>,
    content: Option<PreviewContent>,
}

impl PreviewCache {
    fn new() -> Self {
        Self { path: None, content: None }
    }

    fn get_or_load(&mut self, path: &std::path::Path, max_width: u32, max_height: u32) -> &PreviewContent {
        if self.path.as_ref().map(|p| p.as_path()) != Some(path) {
            self.path = Some(path.to_path_buf());
            self.content = Some(self.load_content(path, max_width, max_height));
        }
        self.content.as_ref().unwrap()
    }

    fn load_content(&mut self, path: &std::path::Path, max_width: u32, max_height: u32) -> PreviewContent {
        // Copy path to owned value for catch_unwind
        let path_owned = path.to_path_buf();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            load_preview_content(&path_owned, max_width, max_height)
        })).unwrap_or_else(|_| PreviewContent::Error("Preview crashed".to_string()))
    }

    fn invalidate(&mut self) {
        self.path = None;
        self.content = None;
    }
}
use std::collections::HashMap;
use input::{Action, Mode, SortMode, handle_key};
use navigation::{Browser, Clipboard, Selection};

use mkframe::{
    App as MkApp, AttachedAnchor, AttachedSurfaceId, Canvas, Color, HAlign, KeyState,
    Rect, SplitDirection, SplitTree, SubsurfaceId, TextColor, TextRenderer, VAlign,
};

enum CommandResult {
    None,
    Redraw,
    ThemeChange(Option<String>),
    Save,
    Exit,
}

struct App {
    config: Config,
    theme: Theme,
    theme_name: Option<String>,
    mode: Mode,
    splits: SplitTree<Browser>,
    clipboard: Clipboard,
    selection: Selection,
    command_buffer: String,
    pending_keys: String,
    overlay_enabled: bool,
    motion_count: Option<usize>,
    pending_theme: Option<Option<String>>,  // Some(None) = default, Some(Some(x)) = named theme
    should_exit: bool,
    openers: Openers,
    // Search
    search_buffer: String,
    last_search: Option<String>,
    // Bookmarks
    bookmarks: HashMap<char, PathBuf>,
    // Sorting
    sort_mode: SortMode,
    sort_reverse: bool,
    // Filter
    filter_pattern: Option<String>,
}

impl App {
    async fn new(start_paths: Vec<PathBuf>, split_direction: SplitDirection) -> Self {
        let config = Config::load().await.unwrap_or_else(|e| {
            eprintln!("failed to load config: {e}");
            std::process::exit(1);
        });

        let show_hidden = config.show_hidden().await;
        let show_parent_entry = config.show_parent_entry().await;
        let overlay_enabled = config.overlay().await.enabled;
        let theme_name = config.theme().await;
        let theme = Theme::load(theme_name.as_deref()).await;
        let openers = Openers::load();

        let mut splits = SplitTree::new();

        if start_paths.is_empty() {
            splits.set_root(Browser::new(show_hidden, show_parent_entry, None));
        } else {
            let mut iter = start_paths.into_iter();
            if let Some(first) = iter.next() {
                splits.set_root(Browser::new(show_hidden, show_parent_entry, Some(first)));
            }
            for path in iter {
                let browser = Browser::new(show_hidden, show_parent_entry, Some(path));
                match split_direction {
                    SplitDirection::Vertical => splits.split_vertical(browser),
                    SplitDirection::Horizontal => splits.split_horizontal(browser),
                };
            }
        }

        Self {
            config,
            theme,
            theme_name,
            mode: Mode::default(),
            splits,
            clipboard: Clipboard::new(),
            selection: Selection::new(),
            command_buffer: String::new(),
            pending_keys: String::new(),
            overlay_enabled,
            motion_count: None,
            pending_theme: None,
            should_exit: false,
            openers,
            search_buffer: String::new(),
            last_search: None,
            bookmarks: HashMap::new(),
            sort_mode: SortMode::default(),
            sort_reverse: false,
            filter_pattern: None,
        }
    }

    /// Get current settings for saving
    fn current_settings(&self) -> SavedSettings {
        // Get show_hidden from the focused browser (or first browser)
        let show_hidden = self.browser().map(|b| b.show_hidden);
        let show_parent_entry = self.browser().map(|b| b.show_parent_entry);

        SavedSettings {
            show_hidden,
            show_parent_entry,
            overlay_enabled: Some(self.overlay_enabled),
            theme: self.theme_name.clone(),
        }
    }

    /// Get a reference to the focused browser.
    fn browser(&self) -> Option<&Browser> {
        self.splits.focused_content()
    }

    /// Get a mutable reference to the focused browser.
    fn browser_mut(&mut self) -> Option<&mut Browser> {
        self.splits.focused_content_mut()
    }

    fn process_key(&mut self, key_str: &str) -> bool {
        // In Normal/Visual mode, accumulate digits for motion count
        if self.mode == Mode::Normal || self.mode == Mode::Visual {
            if let Some(digit) = key_str.chars().next().filter(|c| c.is_ascii_digit()) {
                let d = digit.to_digit(10).unwrap() as usize;
                // Don't start count with 0 (0 could be a motion like go-to-start)
                if self.motion_count.is_some() || d != 0 {
                    let current = self.motion_count.unwrap_or(0);
                    self.motion_count = Some(current * 10 + d);
                    return false;
                }
            }
        }

        let action = handle_key(self.mode, key_str, &self.pending_keys);

        match action {
            Action::Pending => {
                self.pending_keys.push_str(key_str);
                false
            }
            _ => {
                self.pending_keys.clear();
                let count = self.motion_count.take().unwrap_or(1);
                self.execute_with_count(action, count)
            }
        }
    }

    fn execute_with_count(&mut self, action: Action, count: usize) -> bool {
        match &action {
            // Actions that support counts
            Action::MoveCursor(_) | Action::NextDirectory | Action::PrevDirectory => {
                for _ in 0..count {
                    self.execute(action.clone());
                }
                true
            }
            // All other actions ignore count
            _ => self.execute(action),
        }
    }

    fn execute(&mut self, action: Action) -> bool {
        match action {
            Action::None | Action::Pending => false,

            Action::MoveCursor(delta) => {
                let cursor = if let Some(browser) = self.browser_mut() {
                    browser.move_cursor(delta);
                    Some(browser.cursor)
                } else {
                    None
                };
                if self.mode == Mode::Visual {
                    if let Some(c) = cursor {
                        self.selection.add(c);
                    }
                }
                true
            }

            Action::CursorToTop => {
                if let Some(browser) = self.browser_mut() {
                    browser.cursor_to_top();
                }
                true
            }

            Action::CursorToBottom => {
                if let Some(browser) = self.browser_mut() {
                    browser.cursor_to_bottom();
                }
                true
            }

            Action::NextDirectory => {
                if let Some(browser) = self.browser_mut() {
                    browser.next_directory();
                }
                true
            }

            Action::PrevDirectory => {
                if let Some(browser) = self.browser_mut() {
                    browser.prev_directory();
                }
                true
            }

            Action::EnterDirectory => {
                if let Some(browser) = self.browser_mut() {
                    browser.enter_directory();
                }
                true
            }

            Action::ParentDirectory => {
                if let Some(browser) = self.browser_mut() {
                    browser.parent_directory();
                }
                true
            }

            Action::OpenFile => {
                let paths = if self.mode == Mode::Visual {
                    // Visual mode: open all selected files
                    if let Some(browser) = self.browser() {
                        self.selection.to_paths(&browser.entries)
                    } else {
                        Vec::new()
                    }
                } else {
                    // Normal mode: open current file
                    if let Some(browser) = self.browser() {
                        browser.current_entry()
                            .filter(|e| !e.is_dir)
                            .map(|e| vec![e.path.clone()])
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                };

                if !paths.is_empty() {
                    self.openers.open_files(&paths);
                }
                false
            }

            Action::EnterVisualMode => {
                self.mode = Mode::Visual;
                self.selection.clear();
                if let Some(browser) = self.browser() {
                    self.selection.add(browser.cursor);
                }
                true
            }

            Action::ExitVisualMode => {
                self.mode = Mode::Normal;
                self.selection.clear();
                true
            }

            Action::EnterCommandMode => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                true
            }

            Action::CommandAppend(c) => {
                self.command_buffer.push(c);
                true
            }

            Action::CommandBackspace => {
                self.command_buffer.pop();
                true
            }

            Action::CommandExecute => {
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

            Action::CommandCancel => {
                self.command_buffer.clear();
                self.mode = Mode::Normal;
                true
            }

            Action::Yank => {
                if let Some(browser) = self.browser() {
                    if browser.in_archive() {
                        // Yank from archive: store archive path and internal paths
                        if let Some(archive_path) = browser.get_archive_path().cloned() {
                            let file_paths: Vec<String> = if self.selection.is_empty() {
                                browser.current_entry()
                                    .map(|e| vec![e.path.to_string_lossy().to_string()])
                                    .unwrap_or_default()
                            } else {
                                self.selection.to_paths(&browser.entries)
                                    .iter()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .collect()
                            };
                            self.clipboard.yank_from_archive(archive_path, file_paths);
                        }
                    } else {
                        let paths = self.selected_paths();
                        self.clipboard.yank(paths);
                    }
                }
                self.exit_visual_if_active();
                true
            }

            Action::Cut => {
                let paths = self.selected_paths();
                self.clipboard.cut(paths);
                self.exit_visual_if_active();
                true
            }

            Action::Paste => {
                if let Some(browser) = self.browser() {
                    let path = browser.path.clone();
                    let _ = self.clipboard.paste_to(&path);
                }
                if let Some(browser) = self.browser_mut() {
                    browser.refresh();
                }
                true
            }

            Action::Delete => {
                if let Some(browser) = self.browser() {
                    if let Some(entry) = browser.current_entry() {
                        let _ = filesystem::delete(&entry.path);
                    }
                }
                if let Some(browser) = self.browser_mut() {
                    browser.refresh();
                }
                true
            }

            Action::ToggleHidden => {
                if let Some(browser) = self.browser_mut() {
                    browser.toggle_hidden();
                }
                true
            }

            Action::EnableHidden => {
                if let Some(browser) = self.browser_mut() {
                    if !browser.show_hidden {
                        browser.toggle_hidden();
                    }
                }
                true
            }

            Action::DisableHidden => {
                if let Some(browser) = self.browser_mut() {
                    if browser.show_hidden {
                        browser.toggle_hidden();
                    }
                }
                true
            }

            Action::ToggleOverlay => {
                self.overlay_enabled = !self.overlay_enabled;
                true
            }

            Action::EnableOverlay => {
                self.overlay_enabled = true;
                true
            }

            Action::DisableOverlay => {
                self.overlay_enabled = false;
                true
            }

            Action::FocusLeft => {
                self.splits.focus_left();
                true
            }

            Action::FocusRight => {
                self.splits.focus_right();
                true
            }

            Action::FocusUp => {
                self.splits.focus_up();
                true
            }

            Action::FocusDown => {
                self.splits.focus_down();
                true
            }

            Action::SplitVertical => {
                // Clone the current browser's path for the new split
                if let Some(browser) = self.browser() {
                    let path = browser.path.clone();
                    let show_hidden = browser.show_hidden;
                    let show_parent = browser.show_parent_entry;
                    let new_browser = Browser::new(show_hidden, show_parent, Some(path));
                    self.splits.split_vertical(new_browser);
                }
                true
            }

            Action::SplitHorizontal => {
                // Clone the current browser's path for the new split
                if let Some(browser) = self.browser() {
                    let path = browser.path.clone();
                    let show_hidden = browser.show_hidden;
                    let show_parent = browser.show_parent_entry;
                    let new_browser = Browser::new(show_hidden, show_parent, Some(path));
                    self.splits.split_horizontal(new_browser);
                }
                true
            }

            Action::CloseSplit => {
                // Only close if there's more than one split
                if self.splits.len() > 1 {
                    self.splits.close_focused();
                }
                true
            }

            // Search actions
            Action::EnterSearchMode => {
                self.mode = Mode::Search;
                self.search_buffer.clear();
                true
            }

            Action::SearchAppend(c) => {
                self.search_buffer.push(c);
                self.apply_search_filter();
                true
            }

            Action::SearchBackspace => {
                self.search_buffer.pop();
                self.apply_search_filter();
                true
            }

            Action::SearchExecute => {
                self.last_search = if self.search_buffer.is_empty() {
                    None
                } else {
                    Some(self.search_buffer.clone())
                };
                self.mode = Mode::Normal;
                true
            }

            Action::SearchCancel => {
                self.search_buffer.clear();
                // Clear filter when canceling
                if let Some(browser) = self.browser_mut() {
                    browser.clear_filter();
                }
                self.mode = Mode::Normal;
                true
            }

            Action::SearchNext => {
                if let Some(pattern) = self.last_search.clone() {
                    if let Some(browser) = self.browser_mut() {
                        browser.search_next(&pattern);
                    }
                }
                true
            }

            Action::SearchPrev => {
                if let Some(pattern) = self.last_search.clone() {
                    if let Some(browser) = self.browser_mut() {
                        browser.search_prev(&pattern);
                    }
                }
                true
            }

            // Bookmark actions
            Action::SetMark(c) => {
                if let Some(browser) = self.browser() {
                    self.bookmarks.insert(c, browser.path.clone());
                }
                false
            }

            Action::JumpToMark(c) => {
                if let Some(path) = self.bookmarks.get(&c).cloned() {
                    if let Some(browser) = self.browser_mut() {
                        browser.navigate_to(&path);
                    }
                }
                true
            }

            // Sorting actions
            Action::CycleSort => {
                self.sort_mode = self.sort_mode.next();
                let (mode, reverse) = (self.sort_mode, self.sort_reverse);
                if let Some(browser) = self.browser_mut() {
                    browser.set_sort(mode, reverse);
                }
                true
            }

            Action::ReverseSort => {
                self.sort_reverse = !self.sort_reverse;
                let (mode, reverse) = (self.sort_mode, self.sort_reverse);
                if let Some(browser) = self.browser_mut() {
                    browser.set_sort(mode, reverse);
                }
                true
            }

            // Filter action
            Action::ClearFilter => {
                self.filter_pattern = None;
                if let Some(browser) = self.browser_mut() {
                    browser.clear_filter();
                }
                true
            }

            // Trash action
            Action::Trash => {
                let paths = self.selected_paths();
                for path in paths {
                    let _ = filesystem::trash(&path);
                }
                self.exit_visual_if_active();
                if let Some(browser) = self.browser_mut() {
                    browser.refresh();
                }
                true
            }

            // Archive action
            Action::ExtractArchive => {
                if let Some(browser) = self.browser() {
                    if let Some(entry) = browser.current_entry() {
                        let _ = filesystem::extract_archive(&entry.path, &browser.path);
                    }
                }
                if let Some(browser) = self.browser_mut() {
                    browser.refresh();
                }
                true
            }

            // Symlink action
            Action::CreateSymlink => {
                // Create symlink from clipboard to current directory
                if let Some(browser) = self.browser() {
                    let dest_dir = browser.path.clone();
                    for src in &self.clipboard.paths {
                        let _ = filesystem::create_symlink(src, &dest_dir);
                    }
                }
                if let Some(browser) = self.browser_mut() {
                    browser.refresh();
                }
                true
            }
        }
    }

    fn apply_search_filter(&mut self) {
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

    fn current_previewable_path(&self) -> Option<PathBuf> {
        self.browser()
            .and_then(|b| b.current_entry())
            .filter(|e| !e.is_dir)
            .map(|e| e.path.clone())
            .filter(|p| is_image_file(p) || is_text_file(p))
    }

    fn selected_paths(&self) -> Vec<std::path::PathBuf> {
        if let Some(browser) = self.browser() {
            if self.selection.is_empty() {
                browser.current_entry()
                    .map(|e| vec![e.path.clone()])
                    .unwrap_or_default()
            } else {
                self.selection.to_paths(&browser.entries)
            }
        } else {
            Vec::new()
        }
    }

    fn exit_visual_if_active(&mut self) {
        if self.mode == Mode::Visual {
            self.mode = Mode::Normal;
            self.selection.clear();
        }
    }

    fn execute_command(&mut self) -> CommandResult {
        let cmd = self.command_buffer.trim().to_string();

        // Handle :set commands
        if let Some(rest) = cmd.strip_prefix("set ").or_else(|| cmd.strip_prefix("se ")) {
            return self.execute_set_command(rest.trim());
        }

        // Handle :filter <pattern>
        if let Some(pattern) = cmd.strip_prefix("filter ").or_else(|| cmd.strip_prefix("f ")) {
            let pattern = pattern.trim();
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
            return CommandResult::Redraw;
        }

        // Handle :sort <mode>
        if let Some(mode) = cmd.strip_prefix("sort ") {
            let mode = mode.trim();
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
            return CommandResult::Redraw;
        }

        // Handle :chmod <mode>
        if let Some(mode) = cmd.strip_prefix("chmod ") {
            let mode = mode.trim();
            if let Some(browser) = self.browser() {
                if let Some(entry) = browser.current_entry() {
                    let _ = filesystem::chmod(&entry.path, mode);
                }
            }
            if let Some(browser) = self.browser_mut() {
                browser.refresh();
            }
            return CommandResult::Redraw;
        }

        // Handle :rename (bulk rename via $EDITOR)
        if cmd == "rename" || cmd == "bulkrename" {
            self.execute_bulk_rename();
            return CommandResult::Redraw;
        }

        match cmd.as_str() {
            "q" | "quit" => {
                // Close current split, exit if last one
                if self.splits.len() <= 1 {
                    return CommandResult::Exit;
                }
                self.splits.close_focused();
                CommandResult::Redraw
            }
            "qa" | "qall" | "qa!" | "qall!" => {
                // Close all - exit immediately
                CommandResult::Exit
            }
            "sp" | "split" => {
                // Horizontal split
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
                // Vertical split
                if let Some(browser) = self.browser() {
                    let path = browser.path.clone();
                    let show_hidden = browser.show_hidden;
                    let show_parent = browser.show_parent_entry;
                    let new_browser = Browser::new(show_hidden, show_parent, Some(path));
                    self.splits.split_vertical(new_browser);
                }
                CommandResult::Redraw
            }
            "w" | "write" => {
                // Save settings
                CommandResult::Save
            }
            "wq" | "x" => {
                // Save and quit
                if let Err(e) = self.current_settings().save() {
                    eprintln!("failed to save config: {e}");
                }
                CommandResult::Exit
            }
            "filter" | "f" => {
                // Clear filter
                self.filter_pattern = None;
                if let Some(browser) = self.browser_mut() {
                    browser.clear_filter();
                }
                CommandResult::Redraw
            }
            "ln" | "symlink" => {
                // Create symlinks from clipboard
                if let Some(browser) = self.browser() {
                    let dest_dir = browser.path.clone();
                    for src in &self.clipboard.paths {
                        let _ = filesystem::create_symlink(src, &dest_dir);
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

        // Create temp file with file names
        let temp_path = std::env::temp_dir().join("mkfm_rename.txt");
        let names: Vec<String> = paths.iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        if std::fs::write(&temp_path, names.join("\n")).is_err() {
            return;
        }

        // Get editor
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        // Run editor (this will take over the terminal)
        let status = std::process::Command::new(&editor)
            .arg(&temp_path)
            .status();

        if status.is_ok() {
            // Read new names
            if let Ok(content) = std::fs::read_to_string(&temp_path) {
                let new_names: Vec<&str> = content.lines().collect();

                // Rename files
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

        // Refresh
        self.exit_visual_if_active();
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
    }

    fn execute_set_command(&mut self, arg: &str) -> CommandResult {
        // Handle key=value style
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "theme" => {
                    let theme_name = if value.is_empty() || value == "default" {
                        None
                    } else {
                        Some(value.to_string())
                    };
                    return CommandResult::ThemeChange(theme_name);
                }
                _ => {}
            }
            return CommandResult::Redraw;
        }

        // Handle boolean options
        let (negated, option) = if let Some(opt) = arg.strip_prefix("no") {
            (true, opt)
        } else {
            (false, arg)
        };

        match option {
            "hidden" | "hid" => {
                if let Some(browser) = self.browser_mut() {
                    if negated && browser.show_hidden {
                        browser.toggle_hidden();
                    } else if !negated && !browser.show_hidden {
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
}


fn render(
    canvas: &mut Canvas,
    text_renderer: &mut TextRenderer,
    app: &App,
    theme: &Theme,
) {
    let width = canvas.width();
    let height = canvas.height();

    // Colors from theme
    let bg_color = Color::from_rgba8(theme.background.r, theme.background.g, theme.background.b, theme.background.a);
    let fg_color = TextColor::rgb(theme.foreground.r, theme.foreground.g, theme.foreground.b);
    let cursor_bg = Color::from_rgba8(theme.cursor_bg.r, theme.cursor_bg.g, theme.cursor_bg.b, theme.cursor_bg.a);
    let selected_bg = Color::from_rgba8(theme.selection_bg.r, theme.selection_bg.g, theme.selection_bg.b, theme.selection_bg.a);
    let dir_color = TextColor::rgb(theme.directory.r, theme.directory.g, theme.directory.b);
    let header_bg = Color::from_rgba8(theme.header_bg.r, theme.header_bg.g, theme.header_bg.b, theme.header_bg.a);
    let status_bg = Color::from_rgba8(theme.status_bg.r, theme.status_bg.g, theme.status_bg.b, theme.status_bg.a);
    let border_color = Color::from_rgba8(theme.border.r, theme.border.g, theme.border.b, theme.border.a);
    let focused_border = Color::from_rgba8(theme.border_focused.r, theme.border_focused.g, theme.border_focused.b, theme.border_focused.a);

    // Clear background
    canvas.clear(bg_color);

    let font_size = 16.0;
    let line_height = 24;
    let padding = 8;
    let header_height = 32;
    let status_height = 28;

    // Render each split pane
    let bounds = Rect::new(0, 0, width, height.saturating_sub(status_height as u32));
    app.splits.render(bounds, |_leaf_id, pane_rect, browser, is_focused| {
        let pane_x = pane_rect.x;
        let pane_y = pane_rect.y;
        let pane_w = pane_rect.width;
        let pane_h = pane_rect.height;

        // Draw border around pane (1px)
        let border = if is_focused { focused_border } else { border_color };
        canvas.fill_rect(pane_x as f32, pane_y as f32, pane_w as f32, 1.0, border);
        canvas.fill_rect(pane_x as f32, (pane_y + pane_h as i32 - 1) as f32, pane_w as f32, 1.0, border);
        canvas.fill_rect(pane_x as f32, pane_y as f32, 1.0, pane_h as f32, border);
        canvas.fill_rect((pane_x + pane_w as i32 - 1) as f32, pane_y as f32, 1.0, pane_h as f32, border);

        // Inner content area (inset by 1px border)
        let inner_x = pane_x + 1;
        let inner_y = pane_y + 1;
        let inner_w = pane_w.saturating_sub(2);
        let inner_h = pane_h.saturating_sub(2);

        // Header bar with current path
        canvas.fill_rect(inner_x as f32, inner_y as f32, inner_w as f32, header_height as f32, header_bg);
        let header_text = if let Some(archive_path) = browser.get_archive_path() {
            let prefix = browser.get_archive_prefix();
            if prefix.is_empty() {
                format!("[{}]", archive_path.to_string_lossy())
            } else {
                format!("[{}]/{}", archive_path.to_string_lossy(), prefix)
            }
        } else {
            browser.path.to_string_lossy().to_string()
        };
        text_renderer.draw_text_in_rect(
            canvas,
            &header_text,
            Rect::new(inner_x + padding, inner_y, inner_w - padding as u32 * 2, header_height as u32),
            font_size,
            fg_color,
            HAlign::Left,
            VAlign::Center,
        );

        // File list area
        let list_top = inner_y + header_height;
        let list_height = inner_h as i32 - header_height;
        let visible_lines = (list_height / line_height).max(0) as usize;

        // Calculate scroll offset to keep cursor visible
        let scroll_offset = if browser.cursor >= visible_lines && visible_lines > 0 {
            browser.cursor - visible_lines + 1
        } else {
            0
        };

        // Draw file entries
        for (i, entry) in browser.entries.iter().enumerate().skip(scroll_offset).take(visible_lines) {
            let y = list_top + ((i - scroll_offset) as i32 * line_height);
            let is_cursor = i == browser.cursor;
            let is_selected = app.selection.contains(i);

            // Background for cursor/selection
            if is_cursor {
                canvas.fill_rect(inner_x as f32, y as f32, inner_w as f32, line_height as f32, cursor_bg);
            } else if is_selected {
                canvas.fill_rect(inner_x as f32, y as f32, inner_w as f32, line_height as f32, selected_bg);
            }

            // File/directory icon and name (using nerd font icons)
            let icon = if entry.is_dir { &theme.icon_folder } else { &theme.icon_file };
            let display = format!("{} {}", icon, entry.name);
            let color = if entry.is_dir { dir_color } else { fg_color };

            let row_rect = Rect::new(inner_x + padding, y, inner_w - padding as u32 * 2, line_height as u32);
            text_renderer.draw_text_in_rect(
                canvas,
                &display,
                row_rect,
                font_size,
                color,
                HAlign::Left,
                VAlign::Center,
            );

            // Size for files (right-aligned)
            if !entry.is_dir {
                let size_str = filesystem::format_size(entry.size);
                text_renderer.draw_text_in_rect(
                    canvas,
                    &size_str,
                    row_rect,
                    font_size,
                    fg_color,
                    HAlign::Right,
                    VAlign::Center,
                );
            }
        }
    });

    // Status bar (global, below all splits)
    let status_y = height as i32 - status_height;
    canvas.fill_rect(0.0, status_y as f32, width as f32, status_height as f32, status_bg);

    let status_rect = Rect::new(padding, status_y, width - padding as u32 * 2, status_height as u32);

    if app.mode == Mode::Command {
        // Command mode: show only the command line
        let cmd_display = format!(":{}", app.command_buffer);
        text_renderer.draw_text_in_rect(canvas, &cmd_display, status_rect, font_size, fg_color, HAlign::Left, VAlign::Center);
    } else {
        // Normal/Visual mode: show mode indicator and entry count
        let mode_str = app.mode.display();
        text_renderer.draw_text_in_rect(canvas, mode_str, status_rect, font_size, fg_color, HAlign::Left, VAlign::Center);

        // Show count from focused browser
        if let Some(browser) = app.browser() {
            let count = format!("{}/{}", browser.cursor + 1, browser.entries.len());
            text_renderer.draw_text_in_rect(canvas, &count, status_rect, font_size, fg_color, HAlign::Right, VAlign::Center);
        }
    }
}

fn is_image_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp")
    )
}

fn is_text_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(),
        Some("txt" | "md" | "rs" | "toml" | "json" | "yaml" | "yml" | "sh" | "py" | "js" | "ts" | "c" | "h" | "cpp" | "hpp")
    )
}

fn render_preview(
    canvas: &mut Canvas,
    text_renderer: &mut TextRenderer,
    content: &PreviewContent,
) {
    // Transparent background
    canvas.clear(Color::from_rgba8(0, 0, 0, 0));

    match content {
        PreviewContent::Image { data, width: img_w, height: img_h } => {
            // Draw image at origin (surface is sized to fit)
            canvas.draw_rgba(0, 0, *img_w, *img_h, data);
        }
        PreviewContent::Text(lines) => {
            let width = canvas.width();
            let height = canvas.height();
            let font_size = 12.0;
            let line_height = 16;

            for (i, line) in lines.iter().enumerate() {
                let y = i as i32 * line_height;
                if y + line_height > height as i32 {
                    break;
                }
                let line_rect = Rect::new(0, y, width, line_height as u32);
                let display_line = if line.len() > 80 { &line[..80] } else { line.as_str() };
                text_renderer.draw_text_in_rect(
                    canvas,
                    display_line,
                    line_rect,
                    font_size,
                    TextColor::rgb(200, 200, 200),
                    HAlign::Left,
                    VAlign::Top,
                );
            }
        }
        PreviewContent::Unsupported(filename) => {
            let full_rect = Rect::new(0, 0, canvas.width(), canvas.height());
            text_renderer.draw_text_in_rect(
                canvas,
                &format!("No preview for: {}", filename),
                full_rect,
                14.0,
                TextColor::rgb(150, 150, 150),
                HAlign::Center,
                VAlign::Center,
            );
        }
        PreviewContent::Error(msg) => {
            let full_rect = Rect::new(0, 0, canvas.width(), canvas.height());
            text_renderer.draw_text_in_rect(
                canvas,
                msg,
                full_rect,
                14.0,
                TextColor::rgb(200, 100, 100),
                HAlign::Center,
                VAlign::Center,
            );
        }
    }
}

fn parse_args() -> (Vec<PathBuf>, SplitDirection) {
    let mut paths = Vec::new();
    let mut direction = SplitDirection::Vertical; // Default to vertical splits

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--vertical" => {
                direction = SplitDirection::Vertical;
            }
            "-s" | "--horizontal" => {
                direction = SplitDirection::Horizontal;
            }
            "-h" | "--help" => {
                eprintln!("Usage: mkfm [OPTIONS] [PATHS...]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  -v, --vertical     Split panes vertically (side-by-side) [default]");
                eprintln!("  -s, --horizontal   Split panes horizontally (stacked)");
                eprintln!("  -h, --help         Show this help message");
                eprintln!();
                eprintln!("Keybindings:");
                eprintln!("  j/k               Move cursor down/up");
                eprintln!("  h/l               Parent/enter directory");
                eprintln!("  gg/G              Go to top/bottom");
                eprintln!("  v                 Enter visual mode");
                eprintln!("  yy                Yank selected");
                eprintln!("  d                 Cut selected");
                eprintln!("  p                 Paste");
                eprintln!("  =                 Open file with default app");
                eprintln!("  :q                Quit");
                eprintln!();
                eprintln!("Split commands (Ctrl+w prefix):");
                eprintln!("  Ctrl+w v          Create vertical split");
                eprintln!("  Ctrl+w s          Create horizontal split");
                eprintln!("  Ctrl+w h/j/k/l    Focus left/down/up/right pane");
                eprintln!("  Ctrl+w c/q        Close current split");
                eprintln!();
                eprintln!("Settings (:set command):");
                eprintln!("  :set hidden       Show hidden files");
                eprintln!("  :set nohidden     Hide hidden files");
                eprintln!("  :set overlay      Enable preview overlay");
                eprintln!("  :set nooverlay    Disable preview overlay");
                eprintln!("  :set parent       Show parent directory entry (..)");
                eprintln!("  :set noparent     Hide parent directory entry");
                eprintln!("  :set theme=NAME   Change theme (e.g., :set theme=dracula)");
                eprintln!("  :set theme=       Reset to default theme");
                std::process::exit(0);
            }
            path => {
                paths.push(PathBuf::from(path));
            }
        }
        i += 1;
    }

    (paths, direction)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (start_paths, split_direction) = parse_args();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    let mut app = rt.block_on(App::new(start_paths, split_direction));
    let decorations = rt.block_on(app.config.decorations());
    let overlay_config = rt.block_on(app.config.overlay());
    let mut text_renderer = TextRenderer::new();

    let (mut mkapp, mut event_queue) = MkApp::new()?;
    let qh = event_queue.handle();

    let window_id = mkapp.create_window_full(&qh, "mkfm", Some("mkfm"), 800, 600, decorations);
    let mut needs_redraw = true;
    let mut preview_path: Option<PathBuf> = None;
    let mut preview_needs_render = false;
    let mut preview_cache = PreviewCache::new();

    // Use attached surface if available (extends beyond window), fall back to subsurface
    let use_attached_surface = mkapp.has_attached_surface();
    let mut preview_attached: Option<AttachedSurfaceId> = None;
    let mut preview_subsurface: Option<SubsurfaceId> = None;

    while mkapp.running {
        // Flush any pending outgoing messages
        mkapp.flush();

        // Poll with timeout based on key repeat state
        let timeout_ms = mkapp.key_repeat_timeout().map(|t| t as i32).unwrap_or(-1);
        let fd = mkapp.connection_fd();
        let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        unsafe { libc::poll(&mut pfd, 1, timeout_ms); }

        // Read any incoming events and dispatch
        if let Some(guard) = event_queue.prepare_read() {
            let _ = guard.read();
        }
        event_queue.dispatch_pending(&mut mkapp)?;

        // Handle keyboard input
        for event in mkapp.poll_key_events() {
            if event.state != KeyState::Pressed {
                continue;
            }

            if let Some(key_str) = event.to_key_string() {
                if app.process_key(&key_str) {
                    needs_redraw = true;
                }
            }
        }

        // Handle exit request
        if app.should_exit {
            break;
        }

        // Handle pending theme change
        if let Some(new_theme_name) = app.pending_theme.take() {
            app.theme = rt.block_on(Theme::load(new_theme_name.as_deref()));
            app.theme_name = new_theme_name;
            needs_redraw = true;
        }

        // Determine if we should show preview (only for previewable files)
        let current_preview_file = app.current_previewable_path();
        let should_show_preview = app.overlay_enabled && current_preview_file.is_some();

        // Get window dimensions for preview sizing
        let (win_w, win_h) = mkapp.window_size(window_id).unwrap_or((800, 600));

        // Calculate preview dimensions
        let preview_width = overlay_config.max_width.resolve(win_w) as u32;
        let preview_height = overlay_config.max_height.resolve(win_h) as u32;

        // Convert config position to anchor
        let anchor = match overlay_config.position {
            OverlayPosition::Right => AttachedAnchor::Right,
            OverlayPosition::Left => AttachedAnchor::Left,
            OverlayPosition::Top => AttachedAnchor::Top,
            OverlayPosition::Bottom => AttachedAnchor::Bottom,
        };
        let margin = overlay_config.margin.resolve(win_w.min(win_h));
        let offset = overlay_config.offset.resolve(if matches!(overlay_config.position, OverlayPosition::Left | OverlayPosition::Right) { win_h } else { win_w });

        // Update preview surface state (attached surface preferred, subsurface fallback)
        if should_show_preview {
            let have_preview = preview_attached.is_some() || preview_subsurface.is_some();
            let file_changed = preview_path != current_preview_file;

            if !have_preview || file_changed {
                // Load content first to get actual dimensions
                if let Some(ref path) = current_preview_file {
                    let content = preview_cache.get_or_load(path, preview_width, preview_height);
                    let (actual_w, actual_h) = content.dimensions(preview_width, preview_height);

                    // Close old surface if file changed
                    if file_changed {
                        if let Some(attached_id) = preview_attached.take() {
                            mkapp.close_attached_surface(attached_id);
                        }
                        if let Some(subsurface_id) = preview_subsurface.take() {
                            mkapp.close_subsurface(subsurface_id);
                        }
                    }

                    preview_path = current_preview_file.clone();

                    if preview_attached.is_none() && preview_subsurface.is_none() {
                        if use_attached_surface {
                            preview_attached = mkapp.create_attached_surface(
                                &qh,
                                window_id,
                                0, // Position handled by anchor
                                0,
                                actual_w,
                                actual_h,
                            );
                            // Set anchor for automatic positioning
                            if let Some(attached_id) = preview_attached {
                                mkapp.set_attached_surface_anchor(attached_id, anchor, margin, offset);
                            }
                        } else {
                            // Fallback: calculate position manually for subsurface
                            let (preview_x, preview_y) = match overlay_config.position {
                                OverlayPosition::Right => (win_w as i32 + margin, offset),
                                OverlayPosition::Left => (-(actual_w as i32) - margin, offset),
                                OverlayPosition::Top => (offset, -(actual_h as i32) - margin),
                                OverlayPosition::Bottom => (offset, win_h as i32 + margin),
                            };
                            preview_subsurface = mkapp.create_subsurface(
                                &qh,
                                window_id,
                                preview_x,
                                preview_y,
                                actual_w,
                                actual_h,
                            );
                        }
                    }
                    preview_needs_render = true;
                }
            }
            // Note: No position update needed on resize for attached surfaces - anchor handles it
        } else {
            // Close any open preview surface
            if let Some(attached_id) = preview_attached.take() {
                mkapp.close_attached_surface(attached_id);
                preview_path = None;
                preview_needs_render = false;
                preview_cache.invalidate();
            }
            if let Some(subsurface_id) = preview_subsurface.take() {
                mkapp.close_subsurface(subsurface_id);
                preview_path = None;
                preview_needs_render = false;
                preview_cache.invalidate();
            }
        }

        // Render main window when needed
        if mkapp.is_window_dirty(window_id) || needs_redraw {
            mkapp.render_window(window_id, |canvas| {
                render(canvas, &mut text_renderer, &app, &app.theme);
            });
            mkapp.flush();
        }

        // Render preview surface if open (attached or subsurface)
        if let Some(path) = &preview_path {
            if let Some(attached_id) = preview_attached {
                if mkapp.is_attached_surface_dirty(attached_id) || preview_needs_render {
                    let content = preview_cache.get_or_load(path, preview_width - 20, preview_height - 20);
                    mkapp.render_attached_surface(attached_id, |canvas| {
                        render_preview(canvas, &mut text_renderer, content);
                    });
                    mkapp.flush();
                    preview_needs_render = false;
                }
            } else if let Some(subsurface_id) = preview_subsurface {
                if mkapp.is_subsurface_dirty(subsurface_id) || preview_needs_render {
                    let content = preview_cache.get_or_load(path, preview_width - 20, preview_height - 20);
                    mkapp.render_subsurface(subsurface_id, |canvas| {
                        render_preview(canvas, &mut text_renderer, content);
                    });
                    mkapp.flush();
                    preview_needs_render = false;
                }
            }
        }

        needs_redraw = false;
    }

    Ok(())
}
