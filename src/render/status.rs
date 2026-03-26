//! Status bar rendering

use mkui::render::Renderer;
use mkui::style::Style;

use crate::input::Mode;

use super::{RenderColors, RenderLayout};

#[allow(clippy::too_many_arguments)]
pub fn render_status_bar(
    renderer: &mut dyn Renderer,
    mode: &Mode,
    command_buffer: &str,
    _search_buffer: &str,
    _last_search: Option<&str>,
    _search_highlight: bool,
    _search_matches: &[usize],
    _current_match: Option<usize>,
    _active_jobs: usize,
    _failed_jobs: usize,
    cursor_info: Option<(usize, usize)>,
    confirm_message: Option<&str>,
    y: u16,
    width: u16,
    colors: &RenderColors,
    _layout: &RenderLayout,
) {
    let _ = renderer.fill_rect(
        mkui::layout::Rect::new(0, y, width, 1),
        colors.status_bg,
    );

    // Show confirmation prompt if pending
    if let Some(msg) = confirm_message {
        let _ = renderer.move_cursor(1, y);
        let _ = renderer.write_styled(
            msg,
            &Style::new().bg(colors.status_bg).fg(mkui::theme::Color::rgb(255, 200, 100)).bold(true),
        );
        return;
    }

    let mode_str = match mode {
        Mode::Normal => "NORMAL",
        Mode::Visual => "VISUAL",
        Mode::Command => "COMMAND",
        Mode::Search => "SEARCH",
    };

    let _ = renderer.move_cursor(1, y);
    let _ = renderer.write_styled(
        mode_str,
        &Style::new().bg(colors.status_bg).fg(colors.fg).bold(true),
    );

    if *mode == Mode::Command {
        let _ = renderer.move_cursor(10, y);
        let _ = renderer.write_styled(
            &format!(":{command_buffer}"),
            &Style::new().bg(colors.status_bg).fg(colors.fg),
        );
    }

    if let Some((cursor, total)) = cursor_info {
        let pos = format!("{}/{}", cursor + 1, total);
        let pos_x = width.saturating_sub(pos.len() as u16 + 1);
        let _ = renderer.move_cursor(pos_x, y);
        let _ = renderer.write_styled(
            &pos,
            &Style::new().bg(colors.status_bg).fg(colors.fg),
        );
    }
}
