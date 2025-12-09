mod config;
mod filesystem;
mod input;
mod navigation;

use std::path::PathBuf;

use config::{Config, OverlayPosition};
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
use input::{Action, Mode, handle_key};
use navigation::{Browser, Clipboard, Selection};

use mkframe::{
    App as MkApp, AttachedAnchor, AttachedSurfaceId, Canvas, Color, HAlign, Key, KeyState,
    Rect, SubsurfaceId, TextColor, TextRenderer, VAlign,
};

struct App {
    config: Config,
    mode: Mode,
    browser: Browser,
    clipboard: Clipboard,
    selection: Selection,
    command_buffer: String,
    pending_keys: String,
    overlay_enabled: bool,
}

impl App {
    async fn new(start_path: Option<PathBuf>) -> Self {
        let config = Config::load().await.unwrap_or_else(|e| {
            eprintln!("failed to load config: {e}");
            std::process::exit(1);
        });

        let show_hidden = config.show_hidden().await;
        let show_parent_entry = config.show_parent_entry().await;
        let overlay_enabled = config.overlay().await.enabled;

        Self {
            config,
            mode: Mode::default(),
            browser: Browser::new(show_hidden, show_parent_entry, start_path),
            clipboard: Clipboard::new(),
            selection: Selection::new(),
            command_buffer: String::new(),
            pending_keys: String::new(),
            overlay_enabled,
        }
    }

    fn process_key(&mut self, key_str: &str) -> bool {
        let action = handle_key(self.mode, key_str, &self.pending_keys);

        match action {
            Action::Pending => {
                self.pending_keys.push_str(key_str);
                false
            }
            _ => {
                self.pending_keys.clear();
                self.execute(action)
            }
        }
    }

    fn execute(&mut self, action: Action) -> bool {
        match action {
            Action::None | Action::Pending => false,

            Action::MoveCursor(delta) => {
                self.browser.move_cursor(delta);
                if self.mode == Mode::Visual {
                    self.selection.add(self.browser.cursor);
                }
                true
            }

            Action::CursorToTop => {
                self.browser.cursor_to_top();
                true
            }

            Action::CursorToBottom => {
                self.browser.cursor_to_bottom();
                true
            }

            Action::NextDirectory => {
                self.browser.next_directory();
                true
            }

            Action::PrevDirectory => {
                self.browser.prev_directory();
                true
            }

            Action::EnterDirectory => {
                self.browser.enter_directory();
                true
            }

            Action::ParentDirectory => {
                self.browser.parent_directory();
                true
            }

            Action::EnterVisualMode => {
                self.mode = Mode::Visual;
                self.selection.clear();
                self.selection.add(self.browser.cursor);
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
                result
            }

            Action::CommandCancel => {
                self.command_buffer.clear();
                self.mode = Mode::Normal;
                true
            }

            Action::Yank => {
                let paths = self.selected_paths();
                self.clipboard.yank(paths);
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
                let _ = self.clipboard.paste_to(&self.browser.path);
                self.browser.refresh();
                true
            }

            Action::Delete => {
                if let Some(entry) = self.browser.current_entry() {
                    let _ = filesystem::delete(&entry.path);
                    self.browser.refresh();
                }
                true
            }

            Action::ToggleHidden => {
                self.browser.toggle_hidden();
                true
            }

            Action::EnableHidden => {
                if !self.browser.show_hidden {
                    self.browser.toggle_hidden();
                }
                true
            }

            Action::DisableHidden => {
                if self.browser.show_hidden {
                    self.browser.toggle_hidden();
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
        }
    }

    fn current_previewable_path(&self) -> Option<&std::path::Path> {
        self.browser.current_entry()
            .filter(|e| !e.is_dir)
            .map(|e| e.path.as_path())
            .filter(|p| is_image_file(p) || is_text_file(p))
    }

    fn selected_paths(&self) -> Vec<std::path::PathBuf> {
        if self.selection.is_empty() {
            self.browser.current_entry()
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default()
        } else {
            self.selection.to_paths(&self.browser.entries)
        }
    }

    fn exit_visual_if_active(&mut self) {
        if self.mode == Mode::Visual {
            self.mode = Mode::Normal;
            self.selection.clear();
        }
    }

    fn execute_command(&mut self) -> bool {
        match self.command_buffer.trim() {
            "q" | "quit" => std::process::exit(0),
            _ => true,
        }
    }
}

fn key_to_string(key: Key, text: Option<&str>, shift: bool) -> Option<String> {
    // If we have UTF-8 text, use that (handles shift automatically)
    if let Some(t) = text {
        if !t.is_empty() && !t.chars().next().map(|c| c.is_control()).unwrap_or(true) {
            return Some(t.to_string());
        }
    }

    // Otherwise map keys
    let s = match key {
        Key::J => if shift { "J" } else { "j" },
        Key::K => if shift { "K" } else { "k" },
        Key::H => if shift { "H" } else { "h" },
        Key::L => if shift { "L" } else { "l" },
        Key::G => if shift { "G" } else { "g" },
        Key::V => if shift { "V" } else { "v" },
        Key::Y => if shift { "Y" } else { "y" },
        Key::D => if shift { "D" } else { "d" },
        Key::P => if shift { "P" } else { "p" },
        Key::X => if shift { "X" } else { "x" },
        Key::Q => if shift { "Q" } else { "q" },
        Key::Enter => "\n",
        Key::Escape => "\u{1b}",
        Key::Backspace => "\u{8}",
        Key::Period => ".",
        Key::Colon => ":",
        Key::Minus => "-",
        _ => return None,
    };
    Some(s.to_string())
}

fn render(
    canvas: &mut Canvas,
    text_renderer: &mut TextRenderer,
    app: &App,
) {
    let width = canvas.width();
    let height = canvas.height();

    // Colors
    let bg_color = Color::from_rgba8(30, 30, 35, 255);
    let fg_color = TextColor::rgb(220, 220, 220);
    let cursor_bg = Color::from_rgba8(60, 60, 80, 255);
    let selected_bg = Color::from_rgba8(80, 60, 60, 255);
    let dir_color = TextColor::rgb(100, 150, 255);
    let header_bg = Color::from_rgba8(40, 40, 50, 255);
    let status_bg = Color::from_rgba8(50, 50, 60, 255);

    // Clear background
    canvas.clear(bg_color);

    let font_size = 16.0;
    let line_height = 24;
    let padding = 8;
    let header_height = 32;
    let status_height = 28;

    // Header bar with current path
    canvas.fill_rect(0.0, 0.0, width as f32, header_height as f32, header_bg);
    text_renderer.draw_text_in_rect(
        canvas,
        &app.browser.path.to_string_lossy(),
        Rect::new(padding, 0, width - padding as u32 * 2, header_height as u32),
        font_size,
        fg_color,
        HAlign::Left,
        VAlign::Center,
    );

    // File list area
    let list_top = header_height;
    let list_height = height as i32 - header_height - status_height;
    let visible_lines = (list_height / line_height) as usize;

    // Calculate scroll offset to keep cursor visible
    let scroll_offset = if app.browser.cursor >= visible_lines {
        app.browser.cursor - visible_lines + 1
    } else {
        0
    };

    // Draw file entries
    for (i, entry) in app.browser.entries.iter().enumerate().skip(scroll_offset).take(visible_lines) {
        let y = list_top + ((i - scroll_offset) as i32 * line_height);
        let is_cursor = i == app.browser.cursor;
        let is_selected = app.selection.contains(i);

        // Background for cursor/selection
        if is_cursor {
            canvas.fill_rect(0.0, y as f32, width as f32, line_height as f32, cursor_bg);
        } else if is_selected {
            canvas.fill_rect(0.0, y as f32, width as f32, line_height as f32, selected_bg);
        }

        // File/directory indicator and name
        let prefix = if entry.is_dir { "[D] " } else { "    " };
        let display = format!("{}{}", prefix, entry.name);
        let color = if entry.is_dir { dir_color } else { fg_color };

        let row_rect = Rect::new(padding, y, width - padding as u32 * 2, line_height as u32);
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

    // Status bar
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

        let count = format!("{}/{}", app.browser.cursor + 1, app.browser.entries.len());
        text_renderer.draw_text_in_rect(canvas, &count, status_rect, font_size, fg_color, HAlign::Right, VAlign::Center);
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start_path = std::env::args()
        .nth(1)
        .map(PathBuf::from);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    let mut app = rt.block_on(App::new(start_path));
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
        event_queue.blocking_dispatch(&mut mkapp)?;

        // Handle keyboard input
        for event in mkapp.poll_key_events() {
            if event.state != KeyState::Pressed {
                continue;
            }

            if let Some(key_str) = key_to_string(event.key, event.text.as_deref(), event.modifiers.shift) {
                if app.process_key(&key_str) {
                    needs_redraw = true;
                }
            }
        }

        // Determine if we should show preview (only for previewable files)
        let current_preview_file = app.current_previewable_path().map(|p| p.to_path_buf());
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
                render(canvas, &mut text_renderer, &app);
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
