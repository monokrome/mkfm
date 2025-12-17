//! Task/job list pane rendering

use mkframe::{Canvas, Color, HAlign, Rect, TextColor, TextRenderer};

use crate::jobs::{Job, JobStatus};

use super::primitives::{draw_header, draw_row_bg, draw_text};
use super::{RenderColors, RenderLayout};

/// Render the task/error list pane
#[allow(clippy::too_many_arguments)]
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
        render_empty_message(canvas, tr, empty_msg, content_y, w, colors, layout);
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

fn render_empty_message(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    msg: &str,
    y: i32,
    w: u32,
    _colors: &RenderColors,
    layout: &RenderLayout,
) {
    let rect = Rect::new(
        layout.padding,
        y,
        w - layout.padding as u32 * 2,
        layout.line_height as u32,
    );
    let dim = TextColor::rgb(128, 128, 128);
    draw_text(canvas, tr, msg, rect, layout.font_size, dim, HAlign::Center);
}

#[allow(clippy::too_many_arguments)]
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
    draw_text(canvas, tr, &text, rect, layout.font_size, colors.fg, HAlign::Left);

    let prog_x = x + w as i32 - layout.padding - 100;
    let prog_rect = Rect::new(prog_x, y, 100, layout.line_height as u32);
    render_job_status(canvas, tr, &job.status, job.progress, prog_x, y, colors, layout, prog_rect);
}

fn job_status_icon(status: &JobStatus) -> &'static str {
    match status {
        JobStatus::Pending => "\u{f017}",
        JobStatus::Running => "\u{f110}",
        JobStatus::Complete => "\u{f00c}",
        JobStatus::Failed(_) => "\u{f00d}",
    }
}

#[allow(clippy::too_many_arguments)]
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
        JobStatus::Running => render_running_status(canvas, tr, progress, x, y, colors, rect, small_font),
        JobStatus::Complete => render_complete_status(canvas, tr, rect, small_font),
        JobStatus::Failed(msg) => render_failed_status(canvas, tr, msg, rect, small_font),
        JobStatus::Pending => render_pending_status(canvas, tr, colors, rect, small_font),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_running_status(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    progress: Option<f32>,
    x: i32,
    y: i32,
    colors: &RenderColors,
    rect: Rect,
    font_size: f32,
) {
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
        draw_text(canvas, tr, "Running...", rect, font_size, colors.fg, HAlign::Right);
    }
}

fn render_complete_status(canvas: &mut Canvas, tr: &mut TextRenderer, rect: Rect, font_size: f32) {
    draw_text(
        canvas,
        tr,
        "Done",
        rect,
        font_size,
        TextColor::rgb(100, 200, 100),
        HAlign::Right,
    );
}

fn render_failed_status(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    msg: &str,
    rect: Rect,
    font_size: f32,
) {
    let short = if msg.len() > 15 { &msg[..15] } else { msg };
    draw_text(
        canvas,
        tr,
        short,
        rect,
        font_size,
        TextColor::rgb(200, 100, 100),
        HAlign::Right,
    );
}

fn render_pending_status(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    colors: &RenderColors,
    rect: Rect,
    font_size: f32,
) {
    draw_text(canvas, tr, "Pending", rect, font_size, colors.fg, HAlign::Right);
}
