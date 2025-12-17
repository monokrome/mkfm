//! Status bar rendering

use mkframe::{Canvas, HAlign, Rect, TextRenderer};

use crate::input::Mode;

use super::primitives::draw_text;
use super::{RenderColors, RenderLayout};

/// Render the status bar
#[allow(clippy::too_many_arguments)]
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
        Mode::Command => render_command_mode(canvas, tr, command_buffer, rect, colors, layout),
        Mode::Search => render_search_mode(canvas, tr, search_buffer, rect, colors, layout),
        _ => render_normal_mode(
            canvas,
            tr,
            mode,
            last_search,
            search_highlight,
            search_matches,
            current_match,
            active_jobs,
            failed_jobs,
            cursor_info,
            rect,
            colors,
            layout,
        ),
    }
}

fn render_command_mode(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    buffer: &str,
    rect: Rect,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let text = format!(":{}", buffer);
    draw_text(canvas, tr, &text, rect, layout.font_size, colors.fg, HAlign::Left);
}

fn render_search_mode(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    buffer: &str,
    rect: Rect,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let text = format!("/{}", buffer);
    draw_text(canvas, tr, &text, rect, layout.font_size, colors.fg, HAlign::Left);
}

#[allow(clippy::too_many_arguments)]
fn render_normal_mode(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    mode: &Mode,
    last_search: Option<&str>,
    search_highlight: bool,
    search_matches: &[usize],
    current_match: Option<usize>,
    active_jobs: usize,
    failed_jobs: usize,
    cursor_info: Option<(usize, usize)>,
    rect: Rect,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    draw_text(canvas, tr, mode.display(), rect, layout.font_size, colors.fg, HAlign::Left);

    if search_highlight
        && let Some(pattern) = last_search {
            let info = format_search_info(pattern, search_matches, current_match);
            draw_text(canvas, tr, &info, rect, layout.font_size, colors.fg, HAlign::Center);
        }

    let right = format_right_status(active_jobs, failed_jobs, cursor_info);
    if !right.is_empty() {
        draw_text(canvas, tr, &right, rect, layout.font_size, colors.fg, HAlign::Right);
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
