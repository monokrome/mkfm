//! Application rendering — composes mkui rendering for the file manager UI

use mkui::layout::Rect;
use mkui::render::Renderer;

use crate::app::{App, FocusArea};
use crate::config::Theme;
use crate::render;

/// Render the entire application UI
pub fn render_app(renderer: &mut dyn Renderer, app: &App, theme: &Theme) {
    let (width, height) = renderer.dimensions();
    let colors = render::RenderColors::from_theme(theme);
    let layout = render::RenderLayout::default();

    let main_height = height.saturating_sub(layout.status_height);

    // Render split panes
    let bounds = Rect::new(0, 0, width, main_height);
    app.splits
        .render(bounds, |_leaf_id, pane_rect, browser, is_focused| {
            let focused = is_focused && app.focus_area == FocusArea::Splits;
            render::render_browser_pane(
                renderer,
                browser,
                &app.selection,
                app.search_highlight,
                &app.search_matches,
                theme,
                pane_rect.x,
                pane_rect.y,
                pane_rect.width,
                pane_rect.height,
                focused,
                &colors,
                &layout,
                app.icons_enabled,
            );
        });

    // Render status bar
    let cursor_info = app.browser().map(|b| (b.cursor, b.entries.len()));
    render::render_status_bar(
        renderer,
        &app.mode,
        &app.command_buffer,
        &app.search_buffer,
        app.last_search.as_deref(),
        app.search_highlight,
        &app.search_matches,
        app.current_match,
        app.job_queue.active_count(),
        app.job_queue.failed_count(),
        cursor_info,
        main_height,
        width,
        &colors,
        &layout,
    );
}
