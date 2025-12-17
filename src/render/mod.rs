//! Rendering module
//!
//! Split into submodules for reduced complexity.

mod browser;
mod features;
mod primitives;
mod status;
mod tasks;

use mkframe::{Color, TextColor};

use crate::config::Theme;

pub use browser::render_browser_pane;
pub use features::render_feature_panel;
pub use status::render_status_bar;
pub use tasks::render_task_pane;

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
