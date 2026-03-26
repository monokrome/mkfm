//! Rendering module — mkui-based rendering for the file manager

mod browser;
mod status;

use mkui::theme::Color;

use crate::config::Theme;

pub use browser::render_browser_pane;
pub use status::render_status_bar;

/// Pre-converted theme colors for rendering
pub struct RenderColors {
    pub bg: Color,
    pub fg: Color,
    pub cursor_bg: Color,
    pub selected_bg: Color,
    pub search_highlight_bg: Color,
    pub directory: Color,
    pub header_bg: Color,
    pub status_bg: Color,
    pub border: Color,
    pub border_focused: Color,
}

impl RenderColors {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            bg: theme.background.to_color(),
            fg: theme.foreground.to_color(),
            cursor_bg: theme.cursor_bg.to_color(),
            selected_bg: theme.selection_bg.to_color(),
            search_highlight_bg: theme.search_highlight_bg.to_color(),
            directory: theme.directory.to_color(),
            header_bg: theme.header_bg.to_color(),
            status_bg: theme.status_bg.to_color(),
            border: theme.border.to_color(),
            border_focused: theme.border_focused.to_color(),
        }
    }
}

/// Layout constants (cell-based)
#[derive(Clone, Copy)]
pub struct RenderLayout {
    pub header_height: u16,
    pub status_height: u16,
}

impl Default for RenderLayout {
    fn default() -> Self {
        Self {
            header_height: 1,
            status_height: 1,
        }
    }
}
