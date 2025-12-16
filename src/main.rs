#![allow(dead_code)]

mod config;
mod features;
mod filesystem;
mod input;
mod jobs;
mod navigation;

use std::path::{Path, PathBuf};

use config::{Config, Openers, OverlayPosition, SavedSettings, Theme};
use image::GenericImageView;

enum PreviewContent {
    Image {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
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

fn load_svg_preview(
    path: &std::path::Path,
    max_width: u32,
    max_height: u32,
) -> Result<PreviewContent, Box<dyn std::error::Error>> {
    let svg_data = std::fs::read(path)?;
    let tree = resvg::usvg::Tree::from_data(&svg_data, &resvg::usvg::Options::default())?;

    let svg_size = tree.size();
    let svg_w = svg_size.width();
    let svg_h = svg_size.height();

    // Calculate scale to fit within max dimensions
    let scale_x = max_width as f32 / svg_w;
    let scale_y = max_height as f32 / svg_h;
    let scale = scale_x.min(scale_y).min(1.0);

    let target_w = (svg_w * scale).round() as u32;
    let target_h = (svg_h * scale).round() as u32;

    // Create pixmap and render
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(target_w, target_h).ok_or("Failed to create pixmap")?;

    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert from RGBA premultiplied to straight alpha
    let data: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| [p.red(), p.green(), p.blue(), p.alpha()])
        .collect();

    Ok(PreviewContent::Image {
        data,
        width: target_w,
        height: target_h,
    })
}

fn load_preview_content(path: &std::path::Path, max_width: u32, max_height: u32) -> PreviewContent {
    if is_svg_file(path) {
        match load_svg_preview(path, max_width, max_height) {
            Ok(content) => content,
            Err(e) => PreviewContent::Error(format!("SVG load failed: {}", e)),
        }
    } else if is_image_file(path) {
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
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");
        PreviewContent::Unsupported(filename.to_string())
    }
}

struct PreviewCache {
    path: Option<PathBuf>,
    content: Option<PreviewContent>,
}

impl PreviewCache {
    fn new() -> Self {
        Self {
            path: None,
            content: None,
        }
    }

    fn get_or_load(
        &mut self,
        path: &std::path::Path,
        max_width: u32,
        max_height: u32,
    ) -> &PreviewContent {
        if self.path.as_deref() != Some(path) {
            self.path = Some(path.to_path_buf());
            self.content = Some(self.load_content(path, max_width, max_height));
        }
        self.content.as_ref().unwrap()
    }

    fn load_content(
        &mut self,
        path: &std::path::Path,
        max_width: u32,
        max_height: u32,
    ) -> PreviewContent {
        // Copy path to owned value for catch_unwind
        let path_owned = path.to_path_buf();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            load_preview_content(&path_owned, max_width, max_height)
        }))
        .unwrap_or_else(|_| PreviewContent::Error("Preview crashed".to_string()))
    }

    fn invalidate(&mut self) {
        self.path = None;
        self.content = None;
    }
}
use input::{Action, Mode, SortMode, handle_key};
use navigation::{Browser, Clipboard, Selection};
use std::collections::HashMap;

use mkframe::{
    App as MkApp, AttachedAnchor, AttachedSurfaceId, Canvas, Color, HAlign, KeyState,
    PointerButton, PointerEvent, PointerEventKind, Rect, SplitDirection, SplitTree, SubsurfaceId,
    TextColor, TextRenderer, VAlign,
};

enum CommandResult {
    None,
    Redraw,
    ThemeChange(Option<String>),
    Save,
    Exit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FocusArea {
    Splits,
    TaskList,
    FeatureList,
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
    pending_theme: Option<Option<String>>, // Some(None) = default, Some(Some(x)) = named theme
    should_exit: bool,
    openers: Openers,
    // Search
    search_buffer: String,
    last_search: Option<String>,
    search_highlight: bool,
    search_matches: Vec<usize>,
    current_match: Option<usize>,
    // Bookmarks
    bookmarks: HashMap<char, PathBuf>,
    // Sorting
    sort_mode: SortMode,
    sort_reverse: bool,
    // Filter
    filter_pattern: Option<String>,
    // Job queue for background operations
    job_queue: jobs::JobQueue,
    task_list: jobs::TaskListPane,
    error_list: jobs::ErrorListPane,
    runtime: tokio::runtime::Handle,
    // Focus management
    focus_area: FocusArea,
    // Input mode
    vi_mode: bool,
    // Click tracking for double-click detection
    last_click_time: std::time::Instant,
    last_click_pos: (f64, f64),
    // Drag state tracking
    drag_start_pos: Option<(f64, f64)>,
    dragging: bool,
    // Feature availability tracking
    feature_list: features::FeatureList,
    feature_pane: features::FeatureListPane,
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
    fn init_features(&mut self, has_data_device: bool, has_seat: bool, has_attached_surface: bool) {
        use features::*;

        // Vi mode - always available, just a config option
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

        // Drag & Drop
        if has_data_device && has_seat {
            self.feature_list.add(Feature::available(
                FEATURE_DRAG_DROP,
                "Drag files to/from other applications",
            ));
        } else if !has_seat {
            self.feature_list.add(Feature::unavailable(
                FEATURE_DRAG_DROP,
                "Drag files to/from other applications",
                "No Wayland seat available. This may be due to running without a proper Wayland session or missing XDG_RUNTIME_DIR.",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                FEATURE_DRAG_DROP,
                "Drag files to/from other applications",
                "Compositor does not support wl_data_device_manager protocol.",
            ));
        }

        // File preview - always available
        self.feature_list.add(Feature::available(
            FEATURE_PREVIEW,
            "Preview images and text files in overlay",
        ));

        // Overlay extend - attached surfaces for overlay positioning beyond window
        if has_attached_surface {
            self.feature_list.add(Feature::available(
                FEATURE_OVERLAY_EXTEND,
                "Overlay can extend beyond window bounds (wlr-attached-surface)",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                FEATURE_OVERLAY_EXTEND,
                "Overlay can extend beyond window bounds",
                "Compositor does not support wlr-attached-surface protocol. Overlay will use subsurfaces (limited positioning).",
            ));
        }

        // Archive support - check if tools are available
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

        // Trash support - check for trash-cli or gio
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
                "Neither 'trash-put' (trash-cli) nor 'gio' commands found. Files will be permanently deleted.",
            ));
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
            vi: Some(self.vi_mode),
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

    /// Check if a drag should be started and get the files to drag
    /// Returns Some(files) if drag should start, None otherwise
    fn take_drag_files(&mut self) -> Option<Vec<PathBuf>> {
        if self.dragging && self.drag_start_pos.is_some() {
            // Get selected files, or cursor file if no selection
            if let Some(browser) = self.browser() {
                let files: Vec<PathBuf> = if self.selection.is_empty() {
                    // Drag cursor item
                    browser
                        .entries
                        .get(browser.cursor)
                        .map(|e| vec![e.path.clone()])
                        .unwrap_or_default()
                } else {
                    // Drag selected items
                    self.selection.to_paths(&browser.entries)
                };

                if !files.is_empty() {
                    // Clear drag_start_pos so we don't trigger again
                    self.drag_start_pos = None;
                    return Some(files);
                }
            }
        }
        None
    }

    fn process_key(&mut self, key_str: &str) -> bool {
        // In Normal/Visual mode, accumulate digits for motion count
        if (self.mode == Mode::Normal || self.mode == Mode::Visual)
            && let Some(digit) = key_str.chars().next().filter(|c| c.is_ascii_digit())
        {
            let d = digit.to_digit(10).unwrap() as usize;
            // Don't start count with 0 (0 could be a motion like go-to-start)
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

    /// Handle a pointer event (click, double-click, scroll)
    /// Returns true if a redraw is needed
    fn handle_pointer_event(
        &mut self,
        event: &PointerEvent,
        window_width: u32,
        window_height: u32,
        ctrl_held: bool,
    ) -> bool {
        // Layout constants (must match render function)
        let line_height = 24i32;
        let header_height = 32i32;
        let status_height = 28i32;

        // Calculate task/error list pane height
        let list_pane_visible = self.task_list.visible || self.error_list.visible;
        let list_pane_height = if list_pane_visible {
            (window_height as f32 * 0.20).round() as u32
        } else {
            0
        };
        let main_content_height =
            window_height.saturating_sub(status_height as u32 + list_pane_height);

        match &event.kind {
            PointerEventKind::Press(PointerButton::Left) => {
                let x = event.x;
                let y = event.y;

                // Record potential drag start
                self.drag_start_pos = Some((x, y));
                self.dragging = false;

                // Check if click is in main content area (file browser)
                if y < main_content_height as f64 {
                    // Find which split pane was clicked
                    let bounds = Rect::new(0, 0, window_width, main_content_height);
                    if let Some((leaf_id, pane_rect)) = self.splits.find_at_position(bounds, x, y) {
                        // Focus this pane
                        self.splits.set_focused(leaf_id);
                        self.focus_area = FocusArea::Splits;

                        // Calculate entry index from y position
                        let inner_y = pane_rect.y + 1; // 1px border
                        let list_top = inner_y + header_height;
                        let list_height = (pane_rect.height as i32 - 2 - header_height).max(0);
                        let visible_lines = (list_height / line_height).max(0) as usize;

                        // Get relative y within file list
                        let relative_y = y as i32 - list_top;
                        if relative_y >= 0 {
                            let visual_index = (relative_y / line_height) as usize;

                            // Get scroll offset from browser
                            if let Some(browser) = self.splits.get_mut(leaf_id) {
                                let scroll_offset =
                                    if browser.cursor >= visible_lines && visible_lines > 0 {
                                        browser.cursor - visible_lines + 1
                                    } else {
                                        0
                                    };

                                let entry_index = scroll_offset + visual_index;
                                if entry_index < browser.entries.len() {
                                    // Check for double-click
                                    let now = std::time::Instant::now();
                                    let double_click_threshold =
                                        std::time::Duration::from_millis(400);
                                    let distance_threshold = 5.0;

                                    let is_double_click = {
                                        let time_ok = now.duration_since(self.last_click_time)
                                            < double_click_threshold;
                                        let dx = (x - self.last_click_pos.0).abs();
                                        let dy = (y - self.last_click_pos.1).abs();
                                        let pos_ok =
                                            dx < distance_threshold && dy < distance_threshold;
                                        time_ok && pos_ok
                                    };

                                    self.last_click_time = now;
                                    self.last_click_pos = (x, y);

                                    if is_double_click {
                                        // Double-click: enter directory or open file
                                        self.drag_start_pos = None; // Cancel potential drag
                                        browser.cursor = entry_index;
                                        return self.execute(Action::EnterDirectory);
                                    } else if ctrl_held {
                                        // Ctrl+Click: toggle selection, move cursor
                                        browser.cursor = entry_index;
                                        self.selection.toggle(entry_index);
                                    } else {
                                        // Single click: clear selection, move cursor
                                        self.selection.clear();
                                        browser.cursor = entry_index;
                                    }
                                    return true;
                                }
                            }
                        }
                    }
                } else if y < (main_content_height + list_pane_height) as f64 {
                    // Click in task/error list area
                    self.drag_start_pos = None; // No drag from task list
                    self.focus_area = FocusArea::TaskList;
                    return true;
                }
            }
            PointerEventKind::Release(PointerButton::Left) => {
                // Clear drag state
                self.drag_start_pos = None;
                self.dragging = false;
            }
            PointerEventKind::Motion => {
                // Check if we should start a drag
                if let Some((start_x, start_y)) = self.drag_start_pos
                    && !self.dragging
                {
                    let drag_threshold = 8.0;
                    let dx = (event.x - start_x).abs();
                    let dy = (event.y - start_y).abs();
                    let distance = (dx * dx + dy * dy).sqrt();

                    if distance > drag_threshold {
                        // Mark as dragging - main loop will initiate the actual drag
                        self.dragging = true;
                    }
                }
            }
            PointerEventKind::Scroll { dy, .. } => {
                // Scroll wheel
                if let Some(browser) = self.browser_mut() {
                    if *dy < 0 {
                        browser.move_cursor(-3);
                    } else if *dy > 0 {
                        browser.move_cursor(3);
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn execute(&mut self, action: Action) -> bool {
        match action {
            Action::None | Action::Pending => false,

            Action::MoveCursor(delta) => {
                if self.focus_area == FocusArea::FeatureList {
                    // Move cursor in feature list
                    let feature_count = self.feature_list.features.len();
                    self.feature_pane.move_cursor(delta, feature_count);
                } else if self.focus_area == FocusArea::TaskList {
                    // Move cursor in task list
                    let job_count = self.job_queue.all_jobs().len();
                    if job_count > 0 {
                        // Determine which pane's cursor to move
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
                if self.focus_area == FocusArea::FeatureList {
                    // Toggle detail view for the selected feature
                    self.feature_pane.toggle_detail();
                } else if let Some(browser) = self.browser_mut() {
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
                        browser
                            .current_entry()
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
                // Also close feature pane if visible
                if self.feature_pane.visible {
                    if self.feature_pane.showing_detail {
                        // First Escape hides detail, second closes pane
                        self.feature_pane.showing_detail = false;
                    } else {
                        self.feature_pane.hide();
                        self.focus_area = FocusArea::Splits;
                    }
                } else if self.mode == Mode::Visual {
                    self.mode = Mode::Normal;
                    self.selection.clear();
                }
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
                        if let Some(archive_path) =
                            browser.get_archive_path().map(Path::to_path_buf)
                        {
                            let file_paths: Vec<String> = if self.selection.is_empty() {
                                browser
                                    .current_entry()
                                    .map(|e| vec![e.path.to_string_lossy().to_string()])
                                    .unwrap_or_default()
                            } else {
                                self.selection
                                    .to_paths(&browser.entries)
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
                    let dest_dir = browser.path.clone();

                    // Handle archive extraction from clipboard
                    if self.clipboard.is_from_archive() {
                        // TODO: Archive extraction from clipboard still synchronous for now
                        let _ = self.clipboard.paste_to(&dest_dir);
                    } else {
                        // Submit copy/move jobs for each clipboard path
                        for src in &self.clipboard.paths {
                            let Some(name) = src.file_name() else {
                                continue;
                            };
                            let dest = dest_dir.join(name);

                            let kind = if self.clipboard.is_cut {
                                jobs::JobKind::Move {
                                    src: src.clone(),
                                    dest,
                                }
                            } else {
                                jobs::JobKind::Copy {
                                    src: src.clone(),
                                    dest,
                                }
                            };

                            let job_id = self.job_queue.submit(kind.clone());
                            let tx = self.job_queue.sender();
                            self.runtime.spawn(jobs::execute_job(job_id, kind, tx));
                        }

                        // Clear clipboard if cut operation
                        if self.clipboard.is_cut {
                            self.clipboard.paths.clear();
                            self.clipboard.is_cut = false;
                        }
                    }
                }
                if let Some(browser) = self.browser_mut() {
                    browser.refresh();
                }
                true
            }

            Action::Delete => {
                if let Some(browser) = self.browser()
                    && let Some(entry) = browser.current_entry()
                {
                    let _ = filesystem::delete(&entry.path);
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
                if let Some(browser) = self.browser_mut()
                    && !browser.show_hidden
                {
                    browser.toggle_hidden();
                }
                true
            }

            Action::DisableHidden => {
                if let Some(browser) = self.browser_mut()
                    && browser.show_hidden
                {
                    browser.toggle_hidden();
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
                if self.focus_area == FocusArea::Splits {
                    self.splits.focus_left();
                }
                true
            }

            Action::FocusRight => {
                if self.focus_area == FocusArea::Splits {
                    self.splits.focus_right();
                }
                true
            }

            Action::FocusUp => {
                if self.focus_area == FocusArea::TaskList {
                    // Move focus back to splits
                    self.focus_area = FocusArea::Splits;
                } else {
                    self.splits.focus_up();
                }
                true
            }

            Action::FocusDown => {
                let list_visible = self.task_list.visible || self.error_list.visible;
                if self.focus_area == FocusArea::Splits && list_visible {
                    // Move focus to task list
                    self.focus_area = FocusArea::TaskList;
                } else if self.focus_area == FocusArea::Splits {
                    self.splits.focus_down();
                }
                // If already in TaskList, stay there
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
                // Clear live filter first
                if let Some(browser) = self.browser_mut() {
                    browser.clear_filter();
                }

                if self.search_buffer.is_empty() {
                    self.last_search = None;
                    self.search_highlight = false;
                    self.search_matches.clear();
                    self.current_match = None;
                } else {
                    self.last_search = Some(self.search_buffer.clone());
                    // Compute matches for highlighting
                    self.compute_search_matches();
                    self.search_highlight = true;
                    // Set current_match to first match at or after cursor
                    if let Some(browser) = self.browser() {
                        let cursor = browser.cursor;
                        self.current_match =
                            self.search_matches.iter().position(|&i| i >= cursor).or({
                                if self.search_matches.is_empty() {
                                    None
                                } else {
                                    Some(0)
                                }
                            });
                    }
                }
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
                if self.search_highlight && !self.search_matches.is_empty() {
                    // Navigate using match indices
                    let next = self
                        .current_match
                        .map(|i| (i + 1) % self.search_matches.len())
                        .unwrap_or(0);
                    self.current_match = Some(next);
                    let target_cursor = self.search_matches[next];
                    if let Some(browser) = self.browser_mut() {
                        browser.cursor = target_cursor;
                    }
                } else if let Some(pattern) = self.last_search.clone() {
                    // Fallback to old behavior if no highlighting
                    if let Some(browser) = self.browser_mut() {
                        browser.search_next(&pattern);
                    }
                }
                true
            }

            Action::SearchPrev => {
                if self.search_highlight && !self.search_matches.is_empty() {
                    // Navigate using match indices
                    let prev = self
                        .current_match
                        .map(|i| {
                            if i == 0 {
                                self.search_matches.len() - 1
                            } else {
                                i - 1
                            }
                        })
                        .unwrap_or(self.search_matches.len() - 1);
                    self.current_match = Some(prev);
                    let target_cursor = self.search_matches[prev];
                    if let Some(browser) = self.browser_mut() {
                        browser.cursor = target_cursor;
                    }
                } else if let Some(pattern) = self.last_search.clone() {
                    // Fallback to old behavior if no highlighting
                    if let Some(browser) = self.browser_mut() {
                        browser.search_prev(&pattern);
                    }
                }
                true
            }

            Action::ClearSearchHighlight => {
                self.search_highlight = false;
                self.search_matches.clear();
                self.current_match = None;
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
                if let Some(path) = self.bookmarks.get(&c).cloned()
                    && let Some(browser) = self.browser_mut()
                {
                    browser.navigate_to(&path);
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
                    let kind = jobs::JobKind::Trash { path: path.clone() };
                    let job_id = self.job_queue.submit(kind.clone());
                    let tx = self.job_queue.sender();
                    self.runtime.spawn(jobs::execute_job(job_id, kind, tx));
                }
                self.exit_visual_if_active();
                if let Some(browser) = self.browser_mut() {
                    browser.refresh();
                }
                true
            }

            // Archive action
            Action::ExtractArchive => {
                if let Some(browser) = self.browser()
                    && let Some(entry) = browser.current_entry()
                {
                    let kind = jobs::JobKind::Extract {
                        archive: entry.path.clone(),
                        dest: browser.path.clone(),
                    };
                    let job_id = self.job_queue.submit(kind.clone());
                    let tx = self.job_queue.sender();
                    self.runtime.spawn(jobs::execute_job(job_id, kind, tx));
                }
                if let Some(browser) = self.browser_mut() {
                    browser.refresh();
                }
                true
            }

            // Fold (inline expansion) actions
            Action::FoldOpen => {
                if let Some(browser) = self.browser_mut() {
                    let cursor = browser.cursor;
                    browser.expand_directory(cursor, false);
                }
                true
            }

            Action::FoldClose => {
                if let Some(browser) = self.browser_mut() {
                    let cursor = browser.cursor;
                    browser.collapse_directory(cursor, false);
                }
                true
            }

            Action::FoldToggle => {
                if let Some(browser) = self.browser_mut() {
                    let cursor = browser.cursor;
                    browser.toggle_expansion(cursor, false);
                }
                true
            }

            Action::FoldOpenRecursive => {
                if let Some(browser) = self.browser_mut() {
                    let cursor = browser.cursor;
                    browser.expand_directory(cursor, true);
                }
                true
            }

            Action::FoldCloseRecursive => {
                if let Some(browser) = self.browser_mut() {
                    let cursor = browser.cursor;
                    browser.collapse_directory(cursor, true);
                }
                true
            }

            // Task list actions
            Action::NextTask => {
                if !self.task_list.visible {
                    self.task_list.show();
                } else {
                    let job_count = self.job_queue.all_jobs().len();
                    if job_count > 0 && self.task_list.cursor < job_count - 1 {
                        self.task_list.cursor += 1;
                    }
                }
                true
            }

            Action::PrevTask => {
                if !self.task_list.visible {
                    self.task_list.show();
                } else if self.task_list.cursor > 0 {
                    self.task_list.cursor -= 1;
                }
                true
            }

            Action::ToggleTaskList => {
                self.task_list.toggle();
                // Reset focus if no list pane is visible
                if !self.task_list.visible && !self.error_list.visible {
                    self.focus_area = FocusArea::Splits;
                }
                true
            }

            Action::NextError => {
                if !self.error_list.visible {
                    self.error_list.show();
                } else {
                    let error_count = self.job_queue.failed_count();
                    if error_count > 0 && self.error_list.cursor < error_count - 1 {
                        self.error_list.cursor += 1;
                    }
                }
                true
            }

            Action::PrevError => {
                if !self.error_list.visible {
                    self.error_list.show();
                } else if self.error_list.cursor > 0 {
                    self.error_list.cursor -= 1;
                }
                true
            }

            Action::ToggleErrorList => {
                self.error_list.toggle();
                // Reset focus if no list pane is visible
                if !self.task_list.visible && !self.error_list.visible {
                    self.focus_area = FocusArea::Splits;
                }
                true
            }

            Action::ToggleFeatureList => {
                self.feature_pane.toggle();
                if self.feature_pane.visible {
                    self.focus_area = FocusArea::FeatureList;
                } else {
                    self.focus_area = FocusArea::Splits;
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

    fn compute_search_matches(&mut self) {
        self.search_matches.clear();
        if let Some(ref pattern) = self.last_search {
            let pattern_lower = pattern.to_lowercase();
            // Collect matches first to avoid borrow conflict
            let matches: Vec<usize> = if let Some(browser) = self.browser() {
                browser
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| entry.name.to_lowercase().contains(&pattern_lower))
                    .map(|(i, _)| i)
                    .collect()
            } else {
                Vec::new()
            };
            self.search_matches = matches;
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
        if let Some(pattern) = cmd
            .strip_prefix("filter ")
            .or_else(|| cmd.strip_prefix("f "))
        {
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
            if let Some(browser) = self.browser()
                && let Some(entry) = browser.current_entry()
            {
                let _ = filesystem::chmod(&entry.path, mode);
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
        let names: Vec<String> = paths
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        if std::fs::write(&temp_path, names.join("\n")).is_err() {
            return;
        }

        // Get editor
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        // Run editor (this will take over the terminal)
        let status = std::process::Command::new(&editor).arg(&temp_path).status();

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

        // Handle boolean options
        let (negated, option) = if let Some(opt) = arg.strip_prefix("no") {
            (true, opt)
        } else {
            (false, arg)
        };

        match option {
            "hidden" | "hid" => {
                if let Some(browser) = self.browser_mut() {
                    // Toggle if current state doesn't match desired state
                    // negated=true means "nohidden" (want false), negated=false means "hidden" (want true)
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
}

fn render(canvas: &mut Canvas, text_renderer: &mut TextRenderer, app: &App, theme: &Theme) {
    let width = canvas.width();
    let height = canvas.height();

    // Colors from theme
    let bg_color = Color::from_rgba8(
        theme.background.r,
        theme.background.g,
        theme.background.b,
        theme.background.a,
    );
    let fg_color = TextColor::rgb(theme.foreground.r, theme.foreground.g, theme.foreground.b);
    let cursor_bg = Color::from_rgba8(
        theme.cursor_bg.r,
        theme.cursor_bg.g,
        theme.cursor_bg.b,
        theme.cursor_bg.a,
    );
    let selected_bg = Color::from_rgba8(
        theme.selection_bg.r,
        theme.selection_bg.g,
        theme.selection_bg.b,
        theme.selection_bg.a,
    );
    let search_highlight_bg = Color::from_rgba8(
        theme.search_highlight_bg.r,
        theme.search_highlight_bg.g,
        theme.search_highlight_bg.b,
        theme.search_highlight_bg.a,
    );
    let dir_color = TextColor::rgb(theme.directory.r, theme.directory.g, theme.directory.b);
    let header_bg = Color::from_rgba8(
        theme.header_bg.r,
        theme.header_bg.g,
        theme.header_bg.b,
        theme.header_bg.a,
    );
    let status_bg = Color::from_rgba8(
        theme.status_bg.r,
        theme.status_bg.g,
        theme.status_bg.b,
        theme.status_bg.a,
    );
    let border_color = Color::from_rgba8(
        theme.border.r,
        theme.border.g,
        theme.border.b,
        theme.border.a,
    );
    let focused_border = Color::from_rgba8(
        theme.border_focused.r,
        theme.border_focused.g,
        theme.border_focused.b,
        theme.border_focused.a,
    );

    // Clear background
    canvas.clear(bg_color);

    let font_size = 16.0;
    let line_height = 24;
    let padding = 8;
    let header_height = 32;
    let status_height = 28;

    // Calculate task/error list pane height
    let list_pane_visible = app.task_list.visible || app.error_list.visible;
    let list_pane_height = if list_pane_visible {
        (height as f32 * 0.20).round() as u32 // 20% of window height
    } else {
        0
    };

    // Main content height (minus status bar and list pane)
    let main_content_height = height
        .saturating_sub(status_height as u32)
        .saturating_sub(list_pane_height);

    // Render each split pane
    let bounds = Rect::new(0, 0, width, main_content_height);
    app.splits
        .render(bounds, |_leaf_id, pane_rect, browser, is_focused| {
            let pane_x = pane_rect.x;
            let pane_y = pane_rect.y;
            let pane_w = pane_rect.width;
            let pane_h = pane_rect.height;

            // Draw border around pane (1px)
            // Only show focused border if splits area has focus
            let border = if is_focused && app.focus_area == FocusArea::Splits {
                focused_border
            } else {
                border_color
            };
            canvas.fill_rect(pane_x as f32, pane_y as f32, pane_w as f32, 1.0, border);
            canvas.fill_rect(
                pane_x as f32,
                (pane_y + pane_h as i32 - 1) as f32,
                pane_w as f32,
                1.0,
                border,
            );
            canvas.fill_rect(pane_x as f32, pane_y as f32, 1.0, pane_h as f32, border);
            canvas.fill_rect(
                (pane_x + pane_w as i32 - 1) as f32,
                pane_y as f32,
                1.0,
                pane_h as f32,
                border,
            );

            // Inner content area (inset by 1px border)
            let inner_x = pane_x + 1;
            let inner_y = pane_y + 1;
            let inner_w = pane_w.saturating_sub(2);
            let inner_h = pane_h.saturating_sub(2);

            // Header bar with current path
            canvas.fill_rect(
                inner_x as f32,
                inner_y as f32,
                inner_w as f32,
                header_height as f32,
                header_bg,
            );
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
                Rect::new(
                    inner_x + padding,
                    inner_y,
                    inner_w - padding as u32 * 2,
                    header_height as u32,
                ),
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
            for (i, entry) in browser
                .entries
                .iter()
                .enumerate()
                .skip(scroll_offset)
                .take(visible_lines)
            {
                let y = list_top + ((i - scroll_offset) as i32 * line_height);
                let is_cursor = i == browser.cursor;
                let is_selected = app.selection.contains(i);
                let is_search_match = app.search_highlight && app.search_matches.contains(&i);

                // Background priority: cursor > search_match > selection
                if is_cursor {
                    canvas.fill_rect(
                        inner_x as f32,
                        y as f32,
                        inner_w as f32,
                        line_height as f32,
                        cursor_bg,
                    );
                } else if is_search_match {
                    canvas.fill_rect(
                        inner_x as f32,
                        y as f32,
                        inner_w as f32,
                        line_height as f32,
                        search_highlight_bg,
                    );
                } else if is_selected {
                    canvas.fill_rect(
                        inner_x as f32,
                        y as f32,
                        inner_w as f32,
                        line_height as f32,
                        selected_bg,
                    );
                }

                // Calculate indentation based on depth
                let indent = entry.depth as i32 * 16; // 16px per level

                // File/directory icon and name (using nerd font icons)
                let icon = if entry.is_dir {
                    if entry.name != ".." && browser.is_expanded(&entry.path) {
                        &theme.icon_folder_open
                    } else {
                        &theme.icon_folder
                    }
                } else {
                    &theme.icon_file
                };
                let display = format!("{} {}", icon, entry.name);
                let color = if entry.is_dir { dir_color } else { fg_color };

                let row_rect = Rect::new(
                    inner_x + padding + indent,
                    y,
                    (inner_w as i32 - padding * 2 - indent).max(0) as u32,
                    line_height as u32,
                );
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

    // Render task/error list pane if visible
    if list_pane_visible {
        let list_pane_y = main_content_height as i32;
        let list_pane_w = width;
        let list_pane_h = list_pane_height;

        // Draw border at top (highlighted if focused)
        let list_border = if app.focus_area == FocusArea::TaskList {
            focused_border
        } else {
            border_color
        };
        canvas.fill_rect(
            0.0,
            list_pane_y as f32,
            list_pane_w as f32,
            1.0,
            list_border,
        );

        // Background
        canvas.fill_rect(
            0.0,
            (list_pane_y + 1) as f32,
            list_pane_w as f32,
            (list_pane_h - 1) as f32,
            bg_color,
        );

        // Header for list pane
        let list_header_text = if app.task_list.visible && app.error_list.visible {
            "Tasks & Errors"
        } else if app.task_list.visible {
            "Tasks"
        } else {
            "Errors"
        };
        let list_header_rect = Rect::new(
            padding,
            list_pane_y + 1,
            list_pane_w - padding as u32 * 2,
            24,
        );
        canvas.fill_rect(
            0.0,
            (list_pane_y + 1) as f32,
            list_pane_w as f32,
            24.0,
            header_bg,
        );
        text_renderer.draw_text_in_rect(
            canvas,
            list_header_text,
            list_header_rect,
            font_size,
            fg_color,
            HAlign::Left,
            VAlign::Center,
        );

        // List content area
        let list_content_y = list_pane_y + 25;
        let list_content_h = (list_pane_h as i32 - 25).max(0);
        let visible_job_lines = (list_content_h / line_height).max(0) as usize;

        // Determine which jobs to show
        let jobs = app.job_queue.all_jobs();
        let show_jobs: Vec<_> = if app.error_list.visible && !app.task_list.visible {
            // Only errors
            jobs.iter().filter(|j| j.is_failed()).collect()
        } else if app.task_list.visible && !app.error_list.visible {
            // Only active/pending tasks
            jobs.iter()
                .filter(|j| j.is_active() || j.is_complete())
                .collect()
        } else {
            // Both
            jobs.iter().collect()
        };

        // Determine cursor for the visible list
        let cursor = if app.error_list.visible && !app.task_list.visible {
            app.error_list.cursor
        } else {
            app.task_list.cursor
        };

        for (i, job) in show_jobs.iter().enumerate().take(visible_job_lines) {
            let y = list_content_y + (i as i32 * line_height);
            let is_cursor = i == cursor;

            // Highlight cursor row
            if is_cursor {
                canvas.fill_rect(
                    0.0,
                    y as f32,
                    list_pane_w as f32,
                    line_height as f32,
                    cursor_bg,
                );
            }

            // Job status icon
            let status_icon = match &job.status {
                jobs::JobStatus::Pending => "\u{f017}",   // clock
                jobs::JobStatus::Running => "\u{f110}",   // spinner
                jobs::JobStatus::Complete => "\u{f00c}",  // check
                jobs::JobStatus::Failed(_) => "\u{f00d}", // x
            };

            // Job description
            let job_text = format!("{} {}", status_icon, job.description);
            let row_rect = Rect::new(
                padding,
                y,
                (list_pane_w as i32 - padding * 3 - 100) as u32,
                line_height as u32,
            );
            text_renderer.draw_text_in_rect(
                canvas,
                &job_text,
                row_rect,
                font_size,
                fg_color,
                HAlign::Left,
                VAlign::Center,
            );

            // Progress bar or status on the right side
            let progress_rect_x = list_pane_w as i32 - padding - 100;
            let progress_rect_w = 100u32;
            let progress_rect = Rect::new(progress_rect_x, y, progress_rect_w, line_height as u32);

            match &job.status {
                jobs::JobStatus::Running => {
                    if let Some(progress) = job.progress {
                        // Draw progress bar background
                        let bar_y = y + 8;
                        let bar_h = 8;
                        canvas.fill_rect(
                            progress_rect_x as f32,
                            bar_y as f32,
                            progress_rect_w as f32,
                            bar_h as f32,
                            border_color,
                        );
                        // Draw progress fill
                        let fill_w = (progress_rect_w as f32 * progress) as u32;
                        let progress_color = Color::from_rgba8(100, 200, 100, 255);
                        canvas.fill_rect(
                            progress_rect_x as f32,
                            bar_y as f32,
                            fill_w as f32,
                            bar_h as f32,
                            progress_color,
                        );
                    } else {
                        // Show "Running..." text for indeterminate
                        text_renderer.draw_text_in_rect(
                            canvas,
                            "Running...",
                            progress_rect,
                            font_size - 2.0,
                            fg_color,
                            HAlign::Right,
                            VAlign::Center,
                        );
                    }
                }
                jobs::JobStatus::Complete => {
                    let done_color = TextColor::rgb(100, 200, 100);
                    text_renderer.draw_text_in_rect(
                        canvas,
                        "Done",
                        progress_rect,
                        font_size - 2.0,
                        done_color,
                        HAlign::Right,
                        VAlign::Center,
                    );
                }
                jobs::JobStatus::Failed(msg) => {
                    let err_color = TextColor::rgb(200, 100, 100);
                    let short_msg = if msg.len() > 15 { &msg[..15] } else { msg };
                    text_renderer.draw_text_in_rect(
                        canvas,
                        short_msg,
                        progress_rect,
                        font_size - 2.0,
                        err_color,
                        HAlign::Right,
                        VAlign::Center,
                    );
                }
                jobs::JobStatus::Pending => {
                    text_renderer.draw_text_in_rect(
                        canvas,
                        "Pending",
                        progress_rect,
                        font_size - 2.0,
                        fg_color,
                        HAlign::Right,
                        VAlign::Center,
                    );
                }
            }
        }

        // Show empty state if no jobs
        if show_jobs.is_empty() {
            let empty_text = if app.error_list.visible && !app.task_list.visible {
                "No errors"
            } else if app.task_list.visible && !app.error_list.visible {
                "No active tasks"
            } else {
                "No tasks"
            };
            let empty_rect = Rect::new(
                padding,
                list_content_y,
                list_pane_w - padding as u32 * 2,
                line_height as u32,
            );
            let dim_color = TextColor::rgb(128, 128, 128);
            text_renderer.draw_text_in_rect(
                canvas,
                empty_text,
                empty_rect,
                font_size,
                dim_color,
                HAlign::Center,
                VAlign::Center,
            );
        }
    }

    // Render feature list pane as a centered overlay
    if app.feature_pane.visible {
        let feature_count = app.feature_list.features.len();
        let panel_width = (width as f32 * 0.6).min(500.0) as u32;
        let panel_height = if app.feature_pane.showing_detail {
            // Show more height for detail view
            (feature_count as u32 * line_height as u32 + 100).min(height - 100)
        } else {
            (feature_count as u32 * line_height as u32 + 60).min(height - 100)
        };
        let panel_x = (width - panel_width) as i32 / 2;
        let panel_y = (height - panel_height) as i32 / 2;

        // Semi-transparent background overlay
        let overlay_bg = Color::from_rgba8(0, 0, 0, 180);
        canvas.fill_rect(0.0, 0.0, width as f32, height as f32, overlay_bg);

        // Panel background
        let panel_bg = Color::from_rgba8(
            theme.background.r,
            theme.background.g,
            theme.background.b,
            250,
        );
        canvas.fill_rect(
            panel_x as f32,
            panel_y as f32,
            panel_width as f32,
            panel_height as f32,
            panel_bg,
        );

        // Panel border
        let panel_border = if app.focus_area == FocusArea::FeatureList {
            focused_border
        } else {
            border_color
        };
        canvas.fill_rect(
            panel_x as f32,
            panel_y as f32,
            panel_width as f32,
            1.0,
            panel_border,
        );
        canvas.fill_rect(
            panel_x as f32,
            (panel_y + panel_height as i32 - 1) as f32,
            panel_width as f32,
            1.0,
            panel_border,
        );
        canvas.fill_rect(
            panel_x as f32,
            panel_y as f32,
            1.0,
            panel_height as f32,
            panel_border,
        );
        canvas.fill_rect(
            (panel_x + panel_width as i32 - 1) as f32,
            panel_y as f32,
            1.0,
            panel_height as f32,
            panel_border,
        );

        // Panel header
        let header_y = panel_y + 1;
        canvas.fill_rect(
            (panel_x + 1) as f32,
            header_y as f32,
            (panel_width - 2) as f32,
            header_height as f32,
            header_bg,
        );
        let available = app.feature_list.available_count();
        let unavailable = app.feature_list.unavailable_count();
        let header_text = format!(
            "Features ({} available, {} unavailable) - F12 to close",
            available, unavailable
        );
        let header_rect = Rect::new(
            panel_x + padding,
            header_y,
            panel_width - padding as u32 * 2,
            header_height as u32,
        );
        text_renderer.draw_text_in_rect(
            canvas,
            &header_text,
            header_rect,
            font_size,
            fg_color,
            HAlign::Left,
            VAlign::Center,
        );

        // Feature list content
        let content_y = header_y + header_height;
        let content_height = panel_height as i32 - header_height - 2;
        let visible_features = (content_height / line_height).max(0) as usize;

        for (i, feature) in app
            .feature_list
            .features
            .iter()
            .enumerate()
            .take(visible_features)
        {
            let y = content_y + (i as i32 * line_height);
            let is_cursor = i == app.feature_pane.cursor;

            // Highlight cursor row
            if is_cursor {
                canvas.fill_rect(
                    (panel_x + 1) as f32,
                    y as f32,
                    (panel_width - 2) as f32,
                    line_height as f32,
                    cursor_bg,
                );
            }

            // Status icon
            let status_icon = if feature.available {
                "\u{f00c}" // check mark
            } else {
                "\u{f00d}" // x mark
            };
            let status_color = if feature.available {
                TextColor::rgb(100, 200, 100) // green
            } else {
                TextColor::rgb(200, 100, 100) // red
            };

            // Draw status icon
            let icon_rect = Rect::new(panel_x + padding, y, 20, line_height as u32);
            text_renderer.draw_text_in_rect(
                canvas,
                status_icon,
                icon_rect,
                font_size,
                status_color,
                HAlign::Left,
                VAlign::Center,
            );

            // Draw feature name
            let name_rect = Rect::new(
                panel_x + padding + 24,
                y,
                panel_width - padding as u32 * 2 - 24,
                line_height as u32,
            );
            text_renderer.draw_text_in_rect(
                canvas,
                feature.name,
                name_rect,
                font_size,
                fg_color,
                HAlign::Left,
                VAlign::Center,
            );
        }

        // Show detail if requested
        if app.feature_pane.showing_detail {
            if let Some(feature) = app.feature_list.features.get(app.feature_pane.cursor) {
                let detail_y = content_y + (visible_features as i32 * line_height) + 8;
                let detail_h = panel_height as i32 - (detail_y - panel_y);

                if detail_h > 0 {
                    // Separator line
                    canvas.fill_rect(
                        (panel_x + padding) as f32,
                        (detail_y - 4) as f32,
                        (panel_width as i32 - padding * 2) as f32,
                        1.0,
                        border_color,
                    );

                    // Description
                    let desc_rect = Rect::new(
                        panel_x + padding,
                        detail_y,
                        panel_width - padding as u32 * 2,
                        line_height as u32,
                    );
                    text_renderer.draw_text_in_rect(
                        canvas,
                        feature.description,
                        desc_rect,
                        font_size - 1.0,
                        fg_color,
                        HAlign::Left,
                        VAlign::Center,
                    );

                    // Reason if unavailable
                    if let Some(ref reason) = feature.reason {
                        let reason_y = detail_y + line_height;
                        let reason_rect = Rect::new(
                            panel_x + padding,
                            reason_y,
                            panel_width - padding as u32 * 2,
                            (detail_h - line_height).max(line_height) as u32,
                        );
                        let reason_color = TextColor::rgb(200, 150, 100); // orange/warning color
                        text_renderer.draw_text_in_rect(
                            canvas,
                            reason,
                            reason_rect,
                            font_size - 2.0,
                            reason_color,
                            HAlign::Left,
                            VAlign::Top,
                        );
                    }
                }
            }
        } else {
            // Hint at bottom
            let hint_y = panel_y + panel_height as i32 - line_height - 4;
            let hint_rect = Rect::new(
                panel_x + padding,
                hint_y,
                panel_width - padding as u32 * 2,
                line_height as u32,
            );
            let dim_color = TextColor::rgb(128, 128, 128);
            text_renderer.draw_text_in_rect(
                canvas,
                "Press Enter for details, Escape to close",
                hint_rect,
                font_size - 2.0,
                dim_color,
                HAlign::Center,
                VAlign::Center,
            );
        }
    }

    // Status bar (global, below all splits)
    let status_y = height as i32 - status_height;
    canvas.fill_rect(
        0.0,
        status_y as f32,
        width as f32,
        status_height as f32,
        status_bg,
    );

    let status_rect = Rect::new(
        padding,
        status_y,
        width - padding as u32 * 2,
        status_height as u32,
    );

    if app.mode == Mode::Command {
        // Command mode: show only the command line
        let cmd_display = format!(":{}", app.command_buffer);
        text_renderer.draw_text_in_rect(
            canvas,
            &cmd_display,
            status_rect,
            font_size,
            fg_color,
            HAlign::Left,
            VAlign::Center,
        );
    } else if app.mode == Mode::Search {
        // Search mode: show search buffer
        let search_display = format!("/{}", app.search_buffer);
        text_renderer.draw_text_in_rect(
            canvas,
            &search_display,
            status_rect,
            font_size,
            fg_color,
            HAlign::Left,
            VAlign::Center,
        );
    } else {
        // Normal/Visual mode: show mode indicator and entry count
        let mode_str = app.mode.display();
        text_renderer.draw_text_in_rect(
            canvas,
            mode_str,
            status_rect,
            font_size,
            fg_color,
            HAlign::Left,
            VAlign::Center,
        );

        // Show search info in center if highlighting is active
        if app.search_highlight
            && let Some(ref pattern) = app.last_search
        {
            let match_info = if app.search_matches.is_empty() {
                format!("?{} [0/0]", pattern)
            } else {
                let current = app.current_match.map(|i| i + 1).unwrap_or(0);
                format!("?{} [{}/{}]", pattern, current, app.search_matches.len())
            };
            text_renderer.draw_text_in_rect(
                canvas,
                &match_info,
                status_rect,
                font_size,
                fg_color,
                HAlign::Center,
                VAlign::Center,
            );
        }

        // Build right-side status info
        let mut right_info = String::new();

        // Show job count if any active
        let active_jobs = app.job_queue.active_count();
        let failed_jobs = app.job_queue.failed_count();
        if active_jobs > 0 {
            right_info.push_str(&format!("\u{f0f6} {} ", active_jobs)); // nf-fa-file_text_o
        }
        if failed_jobs > 0 {
            right_info.push_str(&format!("\u{f071} {} ", failed_jobs)); // nf-fa-warning
        }

        // Show count from focused browser
        if let Some(browser) = app.browser() {
            right_info.push_str(&format!("{}/{}", browser.cursor + 1, browser.entries.len()));
        }

        if !right_info.is_empty() {
            text_renderer.draw_text_in_rect(
                canvas,
                &right_info,
                status_rect,
                font_size,
                fg_color,
                HAlign::Right,
                VAlign::Center,
            );
        }
    }
}

fn is_image_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg")
    )
}

fn is_svg_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("svg")
    )
}

fn is_text_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some(
            "txt"
                | "md"
                | "rs"
                | "toml"
                | "json"
                | "yaml"
                | "yml"
                | "sh"
                | "py"
                | "js"
                | "ts"
                | "c"
                | "h"
                | "cpp"
                | "hpp"
        )
    )
}

fn render_preview(canvas: &mut Canvas, text_renderer: &mut TextRenderer, content: &PreviewContent) {
    // Transparent background
    canvas.clear(Color::from_rgba8(0, 0, 0, 0));

    match content {
        PreviewContent::Image {
            data,
            width: img_w,
            height: img_h,
        } => {
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
                let display_line = if line.len() > 80 {
                    &line[..80]
                } else {
                    line.as_str()
                };
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

    // Initialize feature availability based on mkframe capabilities
    app.init_features(
        mkapp.has_data_device(),
        mkapp.has_seat(),
        mkapp.has_attached_surface(),
    );

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
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe {
            libc::poll(&mut pfd, 1, timeout_ms);
        }

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

            if let Some(key_str) = event.to_key_string()
                && app.process_key(&key_str)
            {
                needs_redraw = true;
            }
        }

        // Handle pointer input (mouse clicks, scroll)
        let (win_w, win_h) = mkapp.window_size(window_id).unwrap_or((800, 600));
        let modifiers = mkapp.modifiers();
        let ctrl_held = modifiers.ctrl;
        for event in mkapp.poll_pointer_events() {
            if app.handle_pointer_event(&event, win_w, win_h, ctrl_held) {
                needs_redraw = true;
            }
        }

        // Check if we should start a drag operation
        if let Some(files) = app.take_drag_files() {
            let _ = mkapp.start_drag(&qh, window_id, &files);
        }

        // Handle drop events (files dropped from other applications)
        for drop_event in mkapp.poll_drop_events() {
            if !drop_event.files.is_empty() {
                // Copy dropped files to current directory
                if let Some(browser) = app.browser() {
                    let dest_dir = browser.path.clone();
                    for file in drop_event.files {
                        if let Some(file_name) = file.file_name() {
                            let dest = dest_dir.join(file_name);
                            let kind = jobs::JobKind::Copy { src: file, dest };
                            let job_id = app.job_queue.submit(kind.clone());
                            app.runtime.spawn(jobs::execute_job(
                                job_id,
                                kind,
                                app.job_queue.sender(),
                            ));
                        }
                    }
                }
                // Refresh after starting copy jobs
                if let Some(browser) = app.browser_mut() {
                    browser.refresh();
                }
                needs_redraw = true;
            }
        }

        // Poll for job updates (non-blocking)
        let had_active_jobs = app.job_queue.has_active_jobs();
        app.job_queue.poll_updates();

        // If jobs completed, refresh the current browser and trigger redraw
        if had_active_jobs && !app.job_queue.has_active_jobs() {
            if let Some(browser) = app.browser_mut() {
                browser.refresh();
            }
            needs_redraw = true;
        }

        // Also redraw if any jobs are still active (to update progress)
        if app.job_queue.has_active_jobs() {
            needs_redraw = true;
        }

        // Clear completed jobs older than 5 seconds
        app.job_queue.clear_completed(5);

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
        let offset = overlay_config.offset.resolve(
            if matches!(
                overlay_config.position,
                OverlayPosition::Left | OverlayPosition::Right
            ) {
                win_h
            } else {
                win_w
            },
        );

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
                                &qh, window_id, 0, // Position handled by anchor
                                0, actual_w, actual_h,
                            );
                            // Set anchor for automatic positioning
                            if let Some(attached_id) = preview_attached {
                                mkapp.set_attached_surface_anchor(
                                    attached_id,
                                    anchor,
                                    margin,
                                    offset,
                                );
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
                                &qh, window_id, preview_x, preview_y, actual_w, actual_h,
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
                    let content =
                        preview_cache.get_or_load(path, preview_width - 20, preview_height - 20);
                    mkapp.render_attached_surface(attached_id, |canvas| {
                        render_preview(canvas, &mut text_renderer, content);
                    });
                    mkapp.flush();
                    preview_needs_render = false;
                }
            } else if let Some(subsurface_id) = preview_subsurface
                && (mkapp.is_subsurface_dirty(subsurface_id) || preview_needs_render)
            {
                let content =
                    preview_cache.get_or_load(path, preview_width - 20, preview_height - 20);
                mkapp.render_subsurface(subsurface_id, |canvas| {
                    render_preview(canvas, &mut text_renderer, content);
                });
                mkapp.flush();
                preview_needs_render = false;
            }
        }

        needs_redraw = false;
    }

    Ok(())
}
