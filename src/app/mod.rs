//! Application state and logic
//!
//! This module contains the main App struct and its implementation,
//! split across multiple files to keep complexity manageable.

mod execute;
mod handlers;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::{Config, Openers, SavedSettings, Theme};
use crate::features;
use crate::input::{Action, Mode, SortMode, handle_key};
use crate::jobs;
use crate::navigation::{Browser, Clipboard, Selection};

use mkframe::{PointerButton, PointerEvent, PointerEventKind, Rect, SplitDirection, SplitTree};

/// Result of executing a command
pub enum CommandResult {
    None,
    Redraw,
    ThemeChange(Option<String>),
    Save,
    Exit,
}

/// Which area of the UI has focus
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusArea {
    Splits,
    TaskList,
    FeatureList,
}

/// Main application state
pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub theme_name: Option<String>,
    pub mode: Mode,
    pub splits: SplitTree<Browser>,
    pub clipboard: Clipboard,
    pub selection: Selection,
    pub command_buffer: String,
    pub pending_keys: String,
    pub overlay_enabled: bool,
    pub motion_count: Option<usize>,
    pub pending_theme: Option<Option<String>>,
    pub should_exit: bool,
    pub openers: Openers,
    // Search
    pub search_buffer: String,
    pub last_search: Option<String>,
    pub search_highlight: bool,
    pub search_matches: Vec<usize>,
    pub current_match: Option<usize>,
    // Bookmarks
    pub bookmarks: HashMap<char, PathBuf>,
    // Sorting
    pub sort_mode: SortMode,
    pub sort_reverse: bool,
    // Filter
    pub filter_pattern: Option<String>,
    // Job queue
    pub job_queue: jobs::JobQueue,
    pub task_list: jobs::TaskListPane,
    pub error_list: jobs::ErrorListPane,
    pub runtime: tokio::runtime::Handle,
    // Focus
    pub focus_area: FocusArea,
    // Input mode
    pub vi_mode: bool,
    // Click tracking
    pub last_click_time: std::time::Instant,
    pub last_click_pos: (f64, f64),
    // Drag state
    pub drag_start_pos: Option<(f64, f64)>,
    pub dragging: bool,
    // Features
    pub feature_list: features::FeatureList,
    pub feature_pane: features::FeatureListPane,
}

impl App {
    /// Create a new App instance
    pub async fn new(start_paths: Vec<PathBuf>, split_direction: SplitDirection) -> Self {
        let config = Config::load().await.unwrap_or_else(|e| {
            eprintln!("failed to load config: {e}");
            std::process::exit(1);
        });

        let show_hidden = config.show_hidden().await;
        let show_parent_entry = config.show_parent_entry().await;
        let overlay_enabled = config.overlay().await.enabled;
        let vi_mode = config.vi_mode().await;
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
            search_highlight: false,
            search_matches: Vec::new(),
            current_match: None,
            bookmarks: HashMap::new(),
            sort_mode: SortMode::default(),
            sort_reverse: false,
            filter_pattern: None,
            job_queue: jobs::JobQueue::new(),
            task_list: jobs::TaskListPane::new(),
            error_list: jobs::ErrorListPane::new(),
            runtime: tokio::runtime::Handle::current(),
            focus_area: FocusArea::Splits,
            vi_mode,
            last_click_time: std::time::Instant::now(),
            last_click_pos: (0.0, 0.0),
            drag_start_pos: None,
            dragging: false,
            feature_list: features::FeatureList::new(),
            feature_pane: features::FeatureListPane::new(),
        }
    }

    /// Initialize feature availability based on mkframe capabilities
    pub fn init_features(
        &mut self,
        has_data_device: bool,
        has_seat: bool,
        has_attached_surface: bool,
    ) {
        use features::*;

        if self.vi_mode {
            self.feature_list.add(Feature::available(
                FEATURE_VI_MODE,
                "Vi-style keybindings (j/k/h/l, gg, G, etc.)",
            ));
        } else {
            self.feature_list.add(Feature::available(
                FEATURE_VI_MODE,
                "Vi-style keybindings (disabled, set vi=true in config)",
            ));
        }

        if has_data_device && has_seat {
            self.feature_list.add(Feature::available(
                FEATURE_DRAG_DROP,
                "Drag files to/from other applications",
            ));
        } else if !has_seat {
            self.feature_list.add(Feature::unavailable(
                FEATURE_DRAG_DROP,
                "Drag files to/from other applications",
                "No Wayland seat available.",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                FEATURE_DRAG_DROP,
                "Drag files to/from other applications",
                "Compositor does not support wl_data_device_manager protocol.",
            ));
        }

        self.feature_list.add(Feature::available(
            FEATURE_PREVIEW,
            "Preview images and text files in overlay",
        ));

        if has_attached_surface {
            self.feature_list.add(Feature::available(
                FEATURE_OVERLAY_EXTEND,
                "Overlay can extend beyond window bounds (wlr-attached-surface)",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                FEATURE_OVERLAY_EXTEND,
                "Overlay can extend beyond window bounds",
                "Compositor does not support wlr-attached-surface protocol.",
            ));
        }

        let has_tar = std::process::Command::new("tar")
            .arg("--version")
            .output()
            .is_ok();
        let has_unzip = std::process::Command::new("unzip")
            .arg("-v")
            .output()
            .is_ok();
        if has_tar || has_unzip {
            self.feature_list.add(Feature::available(
                FEATURE_ARCHIVE,
                "Browse and extract archives (tar, zip)",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                FEATURE_ARCHIVE,
                "Browse and extract archives (tar, zip)",
                "Neither 'tar' nor 'unzip' commands found in PATH.",
            ));
        }

        let has_trash = std::process::Command::new("trash-put")
            .arg("--version")
            .output()
            .is_ok()
            || std::process::Command::new("gio")
                .arg("help")
                .output()
                .is_ok();
        if has_trash {
            self.feature_list.add(Feature::available(
                FEATURE_TRASH,
                "Move files to trash instead of permanent deletion",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                FEATURE_TRASH,
                "Move files to trash instead of permanent deletion",
                "Neither 'trash-put' nor 'gio' commands found.",
            ));
        }
    }

    /// Get current settings for saving
    pub fn current_settings(&self) -> SavedSettings {
        let show_hidden = self.browser().map(|b| b.show_hidden);
        let show_parent_entry = self.browser().map(|b| b.show_parent_entry);

        SavedSettings {
            show_hidden,
            show_parent_entry,
            overlay_enabled: Some(self.overlay_enabled),
            theme: self.theme_name.clone(),
            vi: Some(self.vi_mode),
        }
    }

    /// Get a reference to the focused browser
    pub fn browser(&self) -> Option<&Browser> {
        self.splits.focused_content()
    }

    /// Get a mutable reference to the focused browser
    pub fn browser_mut(&mut self) -> Option<&mut Browser> {
        self.splits.focused_content_mut()
    }

    /// Check if drag should start, return files to drag
    pub fn take_drag_files(&mut self) -> Option<Vec<PathBuf>> {
        if self.dragging && self.drag_start_pos.is_some() {
            if let Some(browser) = self.browser() {
                let files: Vec<PathBuf> = if self.selection.is_empty() {
                    browser
                        .entries
                        .get(browser.cursor)
                        .map(|e| vec![e.path.clone()])
                        .unwrap_or_default()
                } else {
                    self.selection.to_paths(&browser.entries)
                };

                if !files.is_empty() {
                    self.drag_start_pos = None;
                    return Some(files);
                }
            }
        }
        None
    }

    /// Process a key press
    pub fn process_key(&mut self, key_str: &str) -> bool {
        if (self.mode == Mode::Normal || self.mode == Mode::Visual)
            && let Some(digit) = key_str.chars().next().filter(|c| c.is_ascii_digit())
        {
            let d = digit.to_digit(10).unwrap() as usize;
            if self.motion_count.is_some() || d != 0 {
                let current = self.motion_count.unwrap_or(0);
                self.motion_count = Some(current * 10 + d);
                return false;
            }
        }

        let action = handle_key(self.mode, key_str, &self.pending_keys, self.vi_mode);

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
            Action::MoveCursor(_) | Action::NextDirectory | Action::PrevDirectory => {
                for _ in 0..count {
                    self.execute(action.clone());
                }
                true
            }
            _ => self.execute(action),
        }
    }

    /// Handle pointer events (click, scroll, drag)
    pub fn handle_pointer_event(
        &mut self,
        event: &PointerEvent,
        window_width: u32,
        window_height: u32,
        ctrl_held: bool,
    ) -> bool {
        let line_height = 24i32;
        let header_height = 32i32;
        let status_height = 28i32;

        let list_pane_visible = self.task_list.visible || self.error_list.visible;
        let list_pane_height = if list_pane_visible {
            (window_height as f32 * 0.20).round() as u32
        } else {
            0
        };
        let main_content_height =
            window_height.saturating_sub(status_height as u32 + list_pane_height);

        match &event.kind {
            PointerEventKind::Press(PointerButton::Left) => self.handle_left_click(
                event.x,
                event.y,
                main_content_height,
                list_pane_height,
                header_height,
                line_height,
                window_width,
                ctrl_held,
            ),
            PointerEventKind::Release(PointerButton::Left) => {
                self.drag_start_pos = None;
                self.dragging = false;
                false
            }
            PointerEventKind::Motion => self.handle_motion(event.x, event.y),
            PointerEventKind::Scroll { dy, .. } => self.handle_scroll(*dy as f64),
            _ => false,
        }
    }

    fn handle_left_click(
        &mut self,
        x: f64,
        y: f64,
        main_content_height: u32,
        list_pane_height: u32,
        header_height: i32,
        line_height: i32,
        window_width: u32,
        ctrl_held: bool,
    ) -> bool {
        self.drag_start_pos = Some((x, y));
        self.dragging = false;

        if y < main_content_height as f64 {
            self.handle_browser_click(
                x,
                y,
                main_content_height,
                header_height,
                line_height,
                window_width,
                ctrl_held,
            )
        } else if y < (main_content_height + list_pane_height) as f64 {
            self.drag_start_pos = None;
            self.focus_area = FocusArea::TaskList;
            true
        } else {
            false
        }
    }

    fn handle_browser_click(
        &mut self,
        x: f64,
        y: f64,
        main_content_height: u32,
        header_height: i32,
        line_height: i32,
        window_width: u32,
        ctrl_held: bool,
    ) -> bool {
        let bounds = Rect::new(0, 0, window_width, main_content_height);
        let click_info = if let Some((leaf_id, pane_rect)) =
            self.splits.find_at_position(bounds, x, y)
        {
            self.splits.set_focused(leaf_id);
            self.focus_area = FocusArea::Splits;

            let inner_y = pane_rect.y + 1;
            let list_top = inner_y + header_height;
            let list_height = (pane_rect.height as i32 - 2 - header_height).max(0);
            let visible_lines = (list_height / line_height).max(0) as usize;

            let relative_y = y as i32 - list_top;
            if relative_y >= 0 {
                let visual_index = (relative_y / line_height) as usize;

                if let Some(browser) = self.splits.get_mut(leaf_id) {
                    let scroll_offset = if browser.cursor >= visible_lines && visible_lines > 0 {
                        browser.cursor - visible_lines + 1
                    } else {
                        0
                    };

                    let entry_index = scroll_offset + visual_index;
                    if entry_index < browser.entries.len() {
                        Some((entry_index, ctrl_held))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((entry_index, ctrl_held)) = click_info {
            self.handle_entry_click(x, y, entry_index, ctrl_held)
        } else {
            false
        }
    }

    fn handle_entry_click(&mut self, x: f64, y: f64, entry_index: usize, ctrl_held: bool) -> bool {
        let now = std::time::Instant::now();
        let double_click_threshold = std::time::Duration::from_millis(400);
        let distance_threshold = 5.0;

        let is_double_click = {
            let time_ok = now.duration_since(self.last_click_time) < double_click_threshold;
            let dx = (x - self.last_click_pos.0).abs();
            let dy = (y - self.last_click_pos.1).abs();
            let pos_ok = dx < distance_threshold && dy < distance_threshold;
            time_ok && pos_ok
        };

        self.last_click_time = now;
        self.last_click_pos = (x, y);

        if is_double_click {
            self.drag_start_pos = None;
            if let Some(browser) = self.browser_mut() {
                browser.cursor = entry_index;
            }
            self.execute(Action::EnterDirectory)
        } else if ctrl_held {
            if let Some(browser) = self.browser_mut() {
                browser.cursor = entry_index;
            }
            self.selection.toggle(entry_index);
            true
        } else {
            self.selection.clear();
            if let Some(browser) = self.browser_mut() {
                browser.cursor = entry_index;
            }
            true
        }
    }

    fn handle_motion(&mut self, x: f64, y: f64) -> bool {
        if let Some((start_x, start_y)) = self.drag_start_pos
            && !self.dragging
        {
            let drag_threshold = 8.0;
            let dx = (x - start_x).abs();
            let dy = (y - start_y).abs();
            let distance = (dx * dx + dy * dy).sqrt();

            if distance > drag_threshold {
                self.dragging = true;
            }
        }
        false
    }

    fn handle_scroll(&mut self, dy: f64) -> bool {
        if let Some(browser) = self.browser_mut() {
            if dy < 0.0 {
                browser.move_cursor(-3);
            } else if dy > 0.0 {
                browser.move_cursor(3);
            }
            return true;
        }
        false
    }

    /// Get path of current previewable file
    pub fn current_previewable_path(&self) -> Option<PathBuf> {
        self.browser()
            .and_then(|b| b.current_entry())
            .filter(|e| !e.is_dir)
            .map(|e| e.path.clone())
            .filter(|p| crate::is_image_file(p) || crate::is_text_file(p))
    }

    /// Get selected file paths
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        if let Some(browser) = self.browser() {
            if self.selection.is_empty() {
                browser
                    .current_entry()
                    .map(|e| vec![e.path.clone()])
                    .unwrap_or_default()
            } else {
                self.selection.to_paths(&browser.entries)
            }
        } else {
            Vec::new()
        }
    }

    /// Exit visual mode if active
    pub fn exit_visual_if_active(&mut self) {
        if self.mode == Mode::Visual {
            self.mode = Mode::Normal;
            self.selection.clear();
        }
    }
}
