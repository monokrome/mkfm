//! Application state and logic
//!
//! This module contains the main App struct and its implementation,
//! split across multiple files to keep complexity manageable.

mod bulk_rename;
mod commands;
mod execute;
mod features_init;
mod handlers;
mod pointer;
mod pointer_helpers;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::{Config, Openers, SavedSettings, Theme};
use crate::features;
use crate::input::{Action, Mode, SortMode, handle_key};
use crate::jobs;
use crate::navigation::{Browser, Clipboard, Selection};

use mkframe::{SplitDirection, SplitTree};

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
    pub pre_search_cursor: Option<usize>,
    pub search_active: bool,
    pub search_narrowing: bool,
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
    // Icon display
    pub icons_enabled: bool,
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
        let search_narrowing = config.search_narrowing().await;
        let icons_mode = config.icons().await;
        let icons_enabled = match icons_mode {
            crate::config::IconsMode::Enabled => true,
            crate::config::IconsMode::Disabled => false,
            // For auto mode, default to true (assume Nerd Fonts available)
            // User can set icons = "false" if they don't have them
            crate::config::IconsMode::Auto => true,
        };
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
            pre_search_cursor: None,
            search_active: false,
            search_narrowing,
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
            icons_enabled,
            last_click_time: std::time::Instant::now(),
            last_click_pos: (0.0, 0.0),
            drag_start_pos: None,
            dragging: false,
            feature_list: features::FeatureList::new(),
            feature_pane: features::FeatureListPane::new(),
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
        if self.dragging && self.drag_start_pos.is_some()
            && let Some(browser) = self.browser() {
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

    /// Get path of current previewable file
    pub fn current_previewable_path(&self) -> Option<PathBuf> {
        self.browser()
            .and_then(|b| b.current_entry())
            .filter(|e| !e.is_dir)
            .map(|e| e.path.clone())
            .filter(|p| {
                crate::preview::is_image_file(p)
                    || crate::preview::is_text_file(p)
                    || crate::preview::is_media_file(p)
            })
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
