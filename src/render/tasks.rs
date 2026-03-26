//! Task/job list pane rendering

use mkui::layout::Rect;
use mkui::render::Renderer;
use mkui::style::Style;

use crate::jobs::{Job, JobStatus};

use super::{RenderColors, RenderLayout};

#[allow(clippy::too_many_arguments)]
pub fn render_task_pane(
    renderer: &mut dyn Renderer,
    jobs: &[&Job],
    cursor: usize,
    title: &str,
    empty_msg: &str,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    is_focused: bool,
    colors: &RenderColors,
    _layout: &RenderLayout,
) {
    let border_color = if is_focused {
        colors.border_focused
    } else {
        colors.border
    };

    // Top border
    let _ = renderer.fill_rect(Rect::new(x, y, width, 1), border_color);
    let _ = renderer.move_cursor(x + 1, y);
    let _ = renderer.write_styled(
        title,
        &Style::new().fg(colors.fg).bg(border_color).bold(true),
    );

    // Background
    let content_y = y + 1;
    let content_h = height.saturating_sub(1);
    let _ = renderer.fill_rect(Rect::new(x, content_y, width, content_h), colors.bg);

    if jobs.is_empty() {
        let _ = renderer.move_cursor(x + 2, content_y);
        let _ = renderer.write_styled(empty_msg, &Style::new().fg(colors.fg).dim(true));
        return;
    }

    for (i, job) in jobs.iter().enumerate() {
        if i as u16 >= content_h {
            break;
        }

        let row = content_y + i as u16;
        let is_cursor = i == cursor;

        if is_cursor {
            let _ = renderer.fill_rect(Rect::new(x, row, width, 1), colors.cursor_bg);
        }

        let status_indicator = match &job.status {
            JobStatus::Pending => " ",
            JobStatus::Running => ">",
            JobStatus::Complete => "+",
            JobStatus::Failed(_) => "!",
        };

        let style = match &job.status {
            JobStatus::Failed(_) => Style::new().fg(mkui::theme::Color::rgb(255, 100, 100)),
            JobStatus::Complete => Style::new().fg(mkui::theme::Color::rgb(100, 255, 100)),
            JobStatus::Running => Style::new().fg(colors.fg).bold(true),
            _ => Style::new().fg(colors.fg),
        };

        let _ = renderer.move_cursor(x + 1, row);
        let _ = renderer.write_styled(status_indicator, &style);
        let _ = renderer.write_styled(&format!(" {}", &job.description), &style);
    }
}
