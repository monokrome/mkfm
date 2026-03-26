//! Application rendering — composes mkui rendering for the file manager UI

use mkui::layout::Rect;
use mkui::render::Renderer;

use crate::app::{App, FocusArea};
use crate::config::Theme;
use crate::jobs::Job;
use crate::render;

/// Render the entire application UI
pub fn render_app(renderer: &mut dyn Renderer, app: &App, theme: &Theme) {
    let (width, height) = renderer.dimensions();
    let colors = render::RenderColors::from_theme(theme);
    let layout = render::RenderLayout::default();

    let list_pane_height = calculate_list_pane_height(app, height);
    let main_height = height
        .saturating_sub(layout.status_height)
        .saturating_sub(list_pane_height);

    // Render split panes (file browsers)
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

    // Render task/error pane
    if app.task_list.visible || app.error_list.visible {
        let (jobs, cursor, title, empty_msg) = prepare_task_pane_data(app);
        render::render_task_pane(
            renderer,
            &jobs,
            cursor,
            title,
            empty_msg,
            0,
            main_height,
            width,
            list_pane_height,
            app.focus_area == FocusArea::TaskList,
            &colors,
            &layout,
        );
    }

    // Render status bar
    let status_y = height.saturating_sub(layout.status_height);
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
        status_y,
        width,
        &colors,
        &layout,
    );
}

fn calculate_list_pane_height(app: &App, height: u16) -> u16 {
    if app.task_list.visible || app.error_list.visible {
        (height as f32 * 0.20).round() as u16
    } else {
        0
    }
}

fn prepare_task_pane_data(app: &App) -> (Vec<&Job>, usize, &'static str, &'static str) {
    let all_jobs = app.job_queue.all_jobs();
    if app.error_list.visible && !app.task_list.visible {
        (
            all_jobs.iter().filter(|j| j.is_failed()).collect(),
            app.error_list.cursor,
            "Errors",
            "No errors",
        )
    } else if app.task_list.visible && !app.error_list.visible {
        (
            all_jobs
                .iter()
                .filter(|j| j.is_active() || j.is_complete())
                .collect(),
            app.task_list.cursor,
            "Tasks",
            "No active tasks",
        )
    } else {
        (
            all_jobs.iter().collect(),
            app.task_list.cursor,
            "Tasks & Errors",
            "No tasks",
        )
    }
}
