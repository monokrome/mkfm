//! Browser pane rendering

use mkui::layout::Rect;
use mkui::render::Renderer;
use mkui::style::Style;

use crate::config::Theme;
use crate::navigation::Browser;
use crate::navigation::Selection;

use super::{RenderColors, RenderLayout};

#[allow(clippy::too_many_arguments)]
pub fn render_browser_pane(
    renderer: &mut dyn Renderer,
    browser: &Browser,
    _selection: &Selection,
    _search_highlight: bool,
    _search_matches: &[usize],
    _theme: &Theme,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    _focused: bool,
    colors: &RenderColors,
    layout: &RenderLayout,
    _icons_enabled: bool,
) {
    // Header
    let dir_display = browser.path.display().to_string();
    let header_text = if dir_display.len() > width as usize {
        format!("...{}", &dir_display[dir_display.len() - (width as usize - 3)..])
    } else {
        dir_display
    };

    let _ = renderer.fill_rect(Rect::new(x, y, width, layout.header_height), colors.header_bg);
    let _ = renderer.move_cursor(x, y);
    let _ = renderer.write_styled(&header_text, &Style::new().bg(colors.header_bg).fg(colors.fg));

    // File list
    let list_y = y + layout.header_height;
    let list_height = height.saturating_sub(layout.header_height);

    if browser.entries.is_empty() {
        let _ = renderer.move_cursor(x + 1, list_y);
        let _ = renderer.write_styled("(empty)", &Style::new().fg(colors.fg).dim(true));
        return;
    }

    let visible_count = list_height as usize;
    let scroll_offset = if browser.cursor >= visible_count {
        browser.cursor - visible_count + 1
    } else {
        0
    };

    // Paint background for entire list area (clears old content)
    let _ = renderer.fill_rect(Rect::new(x, list_y, width, list_height), colors.bg);

    for i in 0..visible_count {
        let entry_idx = scroll_offset + i;
        if entry_idx >= browser.entries.len() {
            break;
        }

        let row = list_y + i as u16;
        let entry = &browser.entries[entry_idx];
        let is_cursor = entry_idx == browser.cursor;

        // Row background — cursor highlight or normal bg
        if is_cursor {
            let _ = renderer.fill_rect(Rect::new(x, row, width, 1), colors.cursor_bg);
        }

        let name = &entry.name;
        let style = if entry.is_dir {
            Style::new().fg(colors.directory).bold(true)
        } else {
            Style::new().fg(colors.fg)
        };

        let _ = renderer.move_cursor(x + 1, row);
        let max_name_len = (width as usize).saturating_sub(2);
        let display_name = if name.len() > max_name_len {
            &name[..max_name_len]
        } else {
            name
        };
        let _ = renderer.write_styled(display_name, &style);
    }

}
