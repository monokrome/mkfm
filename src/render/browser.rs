//! File browser pane rendering

use mkframe::{Canvas, HAlign, Rect, TextRenderer};

use crate::config::Theme;
use crate::filesystem;
use crate::navigation::{Browser, Selection};

use super::primitives::{draw_border, draw_header, draw_list_row_bg, draw_text};
use super::{RenderColors, RenderLayout};

/// Context for rendering a file list
pub struct FileListContext<'a> {
    pub browser: &'a Browser,
    pub selection: &'a Selection,
    pub search_highlight: bool,
    pub search_matches: &'a [usize],
    pub theme: &'a Theme,
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
#[allow(clippy::too_many_arguments)]
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
    let border = if is_focused {
        colors.border_focused
    } else {
        colors.border
    };
    draw_border(canvas, x, y, w, h, border);

    let inner_x = x + 1;
    let inner_y = y + 1;
    let inner_w = w.saturating_sub(2);
    let inner_h = h.saturating_sub(2);

    let header_text = browser_header_text(browser);
    draw_header(
        canvas, tr, inner_x, inner_y, inner_w, &header_text, colors, layout,
    );

    let list_y = inner_y + layout.header_height;
    let list_h = inner_h as i32 - layout.header_height;

    let ctx = FileListContext {
        browser,
        selection,
        search_highlight,
        search_matches,
        theme,
    };
    render_file_list(canvas, tr, &ctx, inner_x, list_y, inner_w, list_h, colors, layout);
}

/// Render a file list within the given bounds
#[allow(clippy::too_many_arguments)]
fn render_file_list(
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
        render_file_entry(canvas, tr, entry, ctx.browser, ctx.theme, x, row_y, w, colors, layout);
    }
}

fn calculate_scroll(cursor: usize, visible: usize) -> usize {
    if cursor >= visible && visible > 0 {
        cursor - visible + 1
    } else {
        0
    }
}

#[allow(clippy::too_many_arguments)]
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
    draw_text(canvas, tr, &display, rect, layout.font_size, color, HAlign::Left);

    if !entry.is_dir {
        let size_str = filesystem::format_size(entry.size);
        draw_text(canvas, tr, &size_str, rect, layout.font_size, colors.fg, HAlign::Right);
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
