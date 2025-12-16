use mkframe::{Canvas, Color, HAlign, Rect, TextColor, TextRenderer, VAlign};

use crate::config::Theme;
use crate::features::{Feature, FeatureList, FeatureListPane};
use crate::filesystem;
use crate::input::Mode;
use crate::jobs::{Job, JobStatus};
use crate::navigation::{Browser, Selection};

/// Pre-converted theme colors for rendering
pub struct RenderColors {
    pub bg: Color,
    pub fg: TextColor,
    pub cursor_bg: Color,
    pub selected_bg: Color,
    pub search_highlight_bg: Color,
    pub directory: TextColor,
    pub header_bg: Color,
    pub status_bg: Color,
    pub border: Color,
    pub border_focused: Color,
}

impl RenderColors {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            bg: theme.background.to_color(),
            fg: theme.foreground.to_text_color(),
            cursor_bg: theme.cursor_bg.to_color(),
            selected_bg: theme.selection_bg.to_color(),
            search_highlight_bg: theme.search_highlight_bg.to_color(),
            directory: theme.directory.to_text_color(),
            header_bg: theme.header_bg.to_color(),
            status_bg: theme.status_bg.to_color(),
            border: theme.border.to_color(),
            border_focused: theme.border_focused.to_color(),
        }
    }
}

/// Layout constants for rendering
#[derive(Clone, Copy)]
pub struct RenderLayout {
    pub font_size: f32,
    pub line_height: i32,
    pub padding: i32,
    pub header_height: i32,
    pub status_height: i32,
}

impl Default for RenderLayout {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            line_height: 24,
            padding: 8,
            header_height: 32,
            status_height: 28,
        }
    }
}

// =============================================================================
// Primitive drawing helpers
// =============================================================================

/// Draw a 1px border around a rectangle
pub fn draw_border(canvas: &mut Canvas, x: i32, y: i32, w: u32, h: u32, color: Color) {
    let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
    canvas.fill_rect(x, y, w, 1.0, color);
    canvas.fill_rect(x, y + h - 1.0, w, 1.0, color);
    canvas.fill_rect(x, y, 1.0, h, color);
    canvas.fill_rect(x + w - 1.0, y, 1.0, h, color);
}

/// Draw a filled row background
pub fn draw_row_bg(canvas: &mut Canvas, x: i32, y: i32, w: u32, h: i32, color: Color) {
    canvas.fill_rect(x as f32, y as f32, w as f32, h as f32, color);
}

/// Draw a header bar with background and text
pub fn draw_header(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    x: i32,
    y: i32,
    w: u32,
    text: &str,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    canvas.fill_rect(
        x as f32,
        y as f32,
        w as f32,
        layout.header_height as f32,
        colors.header_bg,
    );
    let rect = Rect::new(
        x + layout.padding,
        y,
        w.saturating_sub(layout.padding as u32 * 2),
        layout.header_height as u32,
    );
    tr.draw_text_in_rect(
        canvas,
        text,
        rect,
        layout.font_size,
        colors.fg,
        HAlign::Left,
        VAlign::Center,
    );
}

/// Draw text in a rect with specified alignment
pub fn draw_text(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    text: &str,
    rect: Rect,
    font_size: f32,
    color: TextColor,
    halign: HAlign,
) {
    tr.draw_text_in_rect(canvas, text, rect, font_size, color, halign, VAlign::Center);
}

// =============================================================================
// Row highlight logic
// =============================================================================

/// Determine and draw the appropriate row background based on state
pub fn draw_list_row_bg(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    w: u32,
    h: i32,
    is_cursor: bool,
    is_selected: bool,
    is_search_match: bool,
    colors: &RenderColors,
) {
    let bg = if is_cursor {
        Some(colors.cursor_bg)
    } else if is_search_match {
        Some(colors.search_highlight_bg)
    } else if is_selected {
        Some(colors.selected_bg)
    } else {
        None
    };
    if let Some(color) = bg {
        draw_row_bg(canvas, x, y, w, h, color);
    }
}

// =============================================================================
// File list rendering
// =============================================================================

/// Context for rendering a file list
pub struct FileListContext<'a> {
    pub browser: &'a Browser,
    pub selection: &'a Selection,
    pub search_highlight: bool,
    pub search_matches: &'a [usize],
    pub theme: &'a Theme,
}

/// Render a file list within the given bounds
pub fn render_file_list(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    ctx: &FileListContext,
    x: i32,
    y: i32,
    w: u32,
    h: i32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let visible_lines = (h / layout.line_height).max(0) as usize;
    let scroll_offset = calculate_scroll(ctx.browser.cursor, visible_lines);

    for (i, entry) in ctx
        .browser
        .entries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_lines)
    {
        let row_y = y + ((i - scroll_offset) as i32 * layout.line_height);
        let is_cursor = i == ctx.browser.cursor;
        let is_selected = ctx.selection.contains(i);
        let is_match = ctx.search_highlight && ctx.search_matches.contains(&i);

        draw_list_row_bg(
            canvas,
            x,
            row_y,
            w,
            layout.line_height,
            is_cursor,
            is_selected,
            is_match,
            colors,
        );
        render_file_entry(
            canvas,
            tr,
            entry,
            ctx.browser,
            ctx.theme,
            x,
            row_y,
            w,
            colors,
            layout,
        );
    }
}

fn calculate_scroll(cursor: usize, visible: usize) -> usize {
    if cursor >= visible && visible > 0 {
        cursor - visible + 1
    } else {
        0
    }
}

fn render_file_entry(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    entry: &filesystem::Entry,
    browser: &Browser,
    theme: &Theme,
    x: i32,
    y: i32,
    w: u32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let indent = entry.depth as i32 * 16;
    let icon = select_icon(entry, browser, theme);
    let display = format!("{} {}", icon, entry.name);
    let color = if entry.is_dir {
        colors.directory
    } else {
        colors.fg
    };

    let row_w = (w as i32 - layout.padding * 2 - indent).max(0) as u32;
    let rect = Rect::new(
        x + layout.padding + indent,
        y,
        row_w,
        layout.line_height as u32,
    );
    draw_text(
        canvas,
        tr,
        &display,
        rect,
        layout.font_size,
        color,
        HAlign::Left,
    );

    if !entry.is_dir {
        let size_str = filesystem::format_size(entry.size);
        draw_text(
            canvas,
            tr,
            &size_str,
            rect,
            layout.font_size,
            colors.fg,
            HAlign::Right,
        );
    }
}

fn select_icon<'a>(entry: &filesystem::Entry, browser: &Browser, theme: &'a Theme) -> &'a str {
    if entry.is_dir {
        if entry.name != ".." && browser.is_expanded(&entry.path) {
            &theme.icon_folder_open
        } else {
            &theme.icon_folder
        }
    } else {
        &theme.icon_file
    }
}

/// Get the header text for a browser pane
pub fn browser_header_text(browser: &Browser) -> String {
    if let Some(archive_path) = browser.get_archive_path() {
        let prefix = browser.get_archive_prefix();
        if prefix.is_empty() {
            format!("[{}]", archive_path.to_string_lossy())
        } else {
            format!("[{}]/{}", archive_path.to_string_lossy(), prefix)
        }
    } else {
        browser.path.to_string_lossy().to_string()
    }
}

/// Render a complete browser pane (border, header, file list)
pub fn render_browser_pane(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    browser: &Browser,
    selection: &Selection,
    search_highlight: bool,
    search_matches: &[usize],
    theme: &Theme,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    is_focused: bool,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    // Border
    let border = if is_focused {
        colors.border_focused
    } else {
        colors.border
    };
    draw_border(canvas, x, y, w, h, border);

    // Inner content area (inset by 1px border)
    let inner_x = x + 1;
    let inner_y = y + 1;
    let inner_w = w.saturating_sub(2);
    let inner_h = h.saturating_sub(2);

    // Header
    let header_text = browser_header_text(browser);
    draw_header(
        canvas,
        tr,
        inner_x,
        inner_y,
        inner_w,
        &header_text,
        colors,
        layout,
    );

    // File list
    let list_y = inner_y + layout.header_height;
    let list_h = inner_h as i32 - layout.header_height;

    let ctx = FileListContext {
        browser,
        selection,
        search_highlight,
        search_matches,
        theme,
    };
    render_file_list(
        canvas, tr, &ctx, inner_x, list_y, inner_w, list_h, colors, layout,
    );
}

// =============================================================================
// Task/Job list rendering
// =============================================================================

/// Render the task/error list pane
pub fn render_task_pane(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    jobs: &[&Job],
    cursor: usize,
    title: &str,
    empty_msg: &str,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    is_focused: bool,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let border = if is_focused {
        colors.border_focused
    } else {
        colors.border
    };
    canvas.fill_rect(x as f32, y as f32, w as f32, 1.0, border);
    canvas.fill_rect(
        x as f32,
        (y + 1) as f32,
        w as f32,
        (h - 1) as f32,
        colors.bg,
    );

    draw_header(canvas, tr, x, y + 1, w, title, colors, layout);

    let content_y = y + 1 + layout.header_height;
    let content_h = h as i32 - 1 - layout.header_height;
    let visible = (content_h / layout.line_height).max(0) as usize;

    if jobs.is_empty() {
        let rect = Rect::new(
            layout.padding,
            content_y,
            w - layout.padding as u32 * 2,
            layout.line_height as u32,
        );
        let dim = TextColor::rgb(128, 128, 128);
        draw_text(
            canvas,
            tr,
            empty_msg,
            rect,
            layout.font_size,
            dim,
            HAlign::Center,
        );
        return;
    }

    for (i, job) in jobs.iter().enumerate().take(visible) {
        let row_y = content_y + (i as i32 * layout.line_height);
        if i == cursor {
            draw_row_bg(canvas, x, row_y, w, layout.line_height, colors.cursor_bg);
        }
        render_job_row(canvas, tr, job, x, row_y, w, colors, layout);
    }
}

fn render_job_row(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    job: &Job,
    x: i32,
    y: i32,
    w: u32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let icon = job_status_icon(&job.status);
    let text = format!("{} {}", icon, job.description);

    let text_w = (w as i32 - layout.padding * 3 - 100) as u32;
    let rect = Rect::new(x + layout.padding, y, text_w, layout.line_height as u32);
    draw_text(
        canvas,
        tr,
        &text,
        rect,
        layout.font_size,
        colors.fg,
        HAlign::Left,
    );

    let prog_x = x + w as i32 - layout.padding - 100;
    let prog_rect = Rect::new(prog_x, y, 100, layout.line_height as u32);
    render_job_status(
        canvas,
        tr,
        &job.status,
        job.progress,
        prog_x,
        y,
        colors,
        layout,
        prog_rect,
    );
}

fn job_status_icon(status: &JobStatus) -> &'static str {
    match status {
        JobStatus::Pending => "\u{f017}",
        JobStatus::Running => "\u{f110}",
        JobStatus::Complete => "\u{f00c}",
        JobStatus::Failed(_) => "\u{f00d}",
    }
}

fn render_job_status(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    status: &JobStatus,
    progress: Option<f32>,
    x: i32,
    y: i32,
    colors: &RenderColors,
    layout: &RenderLayout,
    rect: Rect,
) {
    let small_font = layout.font_size - 2.0;
    match status {
        JobStatus::Running => {
            if let Some(p) = progress {
                let bar_y = y + 8;
                canvas.fill_rect(x as f32, bar_y as f32, 100.0, 8.0, colors.border);
                let fill = (100.0 * p) as u32;
                canvas.fill_rect(
                    x as f32,
                    bar_y as f32,
                    fill as f32,
                    8.0,
                    Color::from_rgba8(100, 200, 100, 255),
                );
            } else {
                draw_text(
                    canvas,
                    tr,
                    "Running...",
                    rect,
                    small_font,
                    colors.fg,
                    HAlign::Right,
                );
            }
        }
        JobStatus::Complete => {
            draw_text(
                canvas,
                tr,
                "Done",
                rect,
                small_font,
                TextColor::rgb(100, 200, 100),
                HAlign::Right,
            );
        }
        JobStatus::Failed(msg) => {
            let short = if msg.len() > 15 { &msg[..15] } else { msg };
            draw_text(
                canvas,
                tr,
                short,
                rect,
                small_font,
                TextColor::rgb(200, 100, 100),
                HAlign::Right,
            );
        }
        JobStatus::Pending => {
            draw_text(
                canvas,
                tr,
                "Pending",
                rect,
                small_font,
                colors.fg,
                HAlign::Right,
            );
        }
    }
}

// =============================================================================
// Feature panel rendering
// =============================================================================

/// Render the feature list overlay panel
pub fn render_feature_panel(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    features: &FeatureList,
    pane: &FeatureListPane,
    width: u32,
    height: u32,
    is_focused: bool,
    theme: &Theme,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let panel_w = (width as f32 * 0.6).min(500.0) as u32;
    let panel_h =
        calculate_panel_height(features.features.len(), pane.showing_detail, height, layout);
    let panel_x = (width - panel_w) as i32 / 2;
    let panel_y = (height - panel_h) as i32 / 2;

    // Dim overlay
    canvas.fill_rect(
        0.0,
        0.0,
        width as f32,
        height as f32,
        Color::from_rgba8(0, 0, 0, 180),
    );

    // Panel background
    let bg = Color::from_rgba8(
        theme.background.r,
        theme.background.g,
        theme.background.b,
        250,
    );
    canvas.fill_rect(
        panel_x as f32,
        panel_y as f32,
        panel_w as f32,
        panel_h as f32,
        bg,
    );

    let border = if is_focused {
        colors.border_focused
    } else {
        colors.border
    };
    draw_border(canvas, panel_x, panel_y, panel_w, panel_h, border);

    // Header
    let header_text = format!(
        "Features ({} available, {} unavailable) - F12 to close",
        features.available_count(),
        features.unavailable_count()
    );
    draw_header(
        canvas,
        tr,
        panel_x + 1,
        panel_y + 1,
        panel_w - 2,
        &header_text,
        colors,
        layout,
    );

    // Content
    let content_y = panel_y + 1 + layout.header_height;
    let content_h = panel_h as i32 - layout.header_height - 2;
    let visible = (content_h / layout.line_height).max(0) as usize;

    for (i, feature) in features.features.iter().enumerate().take(visible) {
        let row_y = content_y + (i as i32 * layout.line_height);
        render_feature_row(
            canvas,
            tr,
            feature,
            i == pane.cursor,
            panel_x + 1,
            row_y,
            panel_w - 2,
            colors,
            layout,
        );
    }

    // Detail or hint
    if pane.showing_detail {
        if let Some(feature) = features.features.get(pane.cursor) {
            render_feature_detail(
                canvas,
                tr,
                feature,
                panel_x,
                content_y + visible as i32 * layout.line_height + 8,
                panel_w,
                panel_h as i32 - (visible as i32 * layout.line_height + 8 - panel_y),
                colors,
                layout,
            );
        }
    } else {
        let hint_y = panel_y + panel_h as i32 - layout.line_height - 4;
        let hint_rect = Rect::new(
            panel_x + layout.padding,
            hint_y,
            panel_w - layout.padding as u32 * 2,
            layout.line_height as u32,
        );
        draw_text(
            canvas,
            tr,
            "Press Enter for details, Escape to close",
            hint_rect,
            layout.font_size - 2.0,
            TextColor::rgb(128, 128, 128),
            HAlign::Center,
        );
    }
}

fn calculate_panel_height(count: usize, detail: bool, max: u32, layout: &RenderLayout) -> u32 {
    let base = if detail { 100 } else { 60 };
    (count as u32 * layout.line_height as u32 + base).min(max - 100)
}

fn render_feature_row(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    feature: &Feature,
    is_cursor: bool,
    x: i32,
    y: i32,
    w: u32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    if is_cursor {
        draw_row_bg(canvas, x, y, w, layout.line_height, colors.cursor_bg);
    }

    let (icon, icon_color) = if feature.available {
        ("\u{f00c}", TextColor::rgb(100, 200, 100))
    } else {
        ("\u{f00d}", TextColor::rgb(200, 100, 100))
    };

    let icon_rect = Rect::new(x + layout.padding, y, 20, layout.line_height as u32);
    draw_text(
        canvas,
        tr,
        icon,
        icon_rect,
        layout.font_size,
        icon_color,
        HAlign::Left,
    );

    let name_rect = Rect::new(
        x + layout.padding + 24,
        y,
        w - layout.padding as u32 * 2 - 24,
        layout.line_height as u32,
    );
    draw_text(
        canvas,
        tr,
        feature.name,
        name_rect,
        layout.font_size,
        colors.fg,
        HAlign::Left,
    );
}

fn render_feature_detail(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    feature: &Feature,
    x: i32,
    y: i32,
    w: u32,
    h: i32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    if h <= 0 {
        return;
    }

    // Separator
    canvas.fill_rect(
        (x + layout.padding) as f32,
        (y - 4) as f32,
        (w as i32 - layout.padding * 2) as f32,
        1.0,
        colors.border,
    );

    // Description
    let desc_rect = Rect::new(
        x + layout.padding,
        y,
        w - layout.padding as u32 * 2,
        layout.line_height as u32,
    );
    draw_text(
        canvas,
        tr,
        feature.description,
        desc_rect,
        layout.font_size - 1.0,
        colors.fg,
        HAlign::Left,
    );

    // Reason if unavailable
    if let Some(ref reason) = feature.reason {
        let reason_rect = Rect::new(
            x + layout.padding,
            y + layout.line_height,
            w - layout.padding as u32 * 2,
            (h - layout.line_height).max(layout.line_height) as u32,
        );
        tr.draw_text_in_rect(
            canvas,
            reason,
            reason_rect,
            layout.font_size - 2.0,
            TextColor::rgb(200, 150, 100),
            HAlign::Left,
            VAlign::Top,
        );
    }
}

// =============================================================================
// Status bar rendering
// =============================================================================

/// Render the status bar
pub fn render_status_bar(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    mode: &Mode,
    command_buffer: &str,
    search_buffer: &str,
    last_search: Option<&str>,
    search_highlight: bool,
    search_matches: &[usize],
    current_match: Option<usize>,
    active_jobs: usize,
    failed_jobs: usize,
    cursor_info: Option<(usize, usize)>,
    y: i32,
    w: u32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    canvas.fill_rect(
        0.0,
        y as f32,
        w as f32,
        layout.status_height as f32,
        colors.status_bg,
    );

    let rect = Rect::new(
        layout.padding,
        y,
        w - layout.padding as u32 * 2,
        layout.status_height as u32,
    );

    match mode {
        Mode::Command => {
            let text = format!(":{}", command_buffer);
            draw_text(
                canvas,
                tr,
                &text,
                rect,
                layout.font_size,
                colors.fg,
                HAlign::Left,
            );
        }
        Mode::Search => {
            let text = format!("/{}", search_buffer);
            draw_text(
                canvas,
                tr,
                &text,
                rect,
                layout.font_size,
                colors.fg,
                HAlign::Left,
            );
        }
        _ => {
            draw_text(
                canvas,
                tr,
                mode.display(),
                rect,
                layout.font_size,
                colors.fg,
                HAlign::Left,
            );

            if search_highlight {
                if let Some(pattern) = last_search {
                    let info = format_search_info(pattern, search_matches, current_match);
                    draw_text(
                        canvas,
                        tr,
                        &info,
                        rect,
                        layout.font_size,
                        colors.fg,
                        HAlign::Center,
                    );
                }
            }

            let right = format_right_status(active_jobs, failed_jobs, cursor_info);
            if !right.is_empty() {
                draw_text(
                    canvas,
                    tr,
                    &right,
                    rect,
                    layout.font_size,
                    colors.fg,
                    HAlign::Right,
                );
            }
        }
    }
}

fn format_search_info(pattern: &str, matches: &[usize], current: Option<usize>) -> String {
    if matches.is_empty() {
        format!("?{} [0/0]", pattern)
    } else {
        let cur = current.map(|i| i + 1).unwrap_or(0);
        format!("?{} [{}/{}]", pattern, cur, matches.len())
    }
}

fn format_right_status(active: usize, failed: usize, cursor: Option<(usize, usize)>) -> String {
    let mut s = String::new();
    if active > 0 {
        s.push_str(&format!("\u{f0f6} {} ", active));
    }
    if failed > 0 {
        s.push_str(&format!("\u{f071} {} ", failed));
    }
    if let Some((cur, total)) = cursor {
        s.push_str(&format!("{}/{}", cur + 1, total));
    }
    s
}

// =============================================================================
// Format helpers
// =============================================================================

/// Format file size in human-readable format
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Truncate text with ellipsis if it exceeds max_chars
pub fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}
