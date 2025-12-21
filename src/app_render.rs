//! Application rendering functions

use mkframe::{Canvas, Rect, TextRenderer};

use crate::app::{App, FocusArea};
use crate::config::Theme;
use crate::jobs::Job;
use crate::render;

/// Render the entire application UI
pub fn render_app(
    canvas: &mut Canvas,
    text_renderer: &mut TextRenderer,
    app: &App,
    theme: &Theme,
) {
    let width = canvas.width();
    let height = canvas.height();

    let colors = render::RenderColors::from_theme(theme);
    let layout = render::RenderLayout::default();

    canvas.clear(colors.bg);

    let list_pane_height = calculate_list_pane_height(app, height);
    let main_content_height = height
        .saturating_sub(layout.status_height as u32)
        .saturating_sub(list_pane_height);

    render_split_panes(canvas, text_renderer, app, theme, &colors, &layout, width, main_content_height);

    if app.task_list.visible || app.error_list.visible {
        render_task_pane(canvas, text_renderer, app, &colors, &layout, main_content_height, width, list_pane_height);
    }

    if app.feature_pane.visible {
        render::render_feature_panel(
            canvas,
            text_renderer,
            &app.feature_list,
            &app.feature_pane,
            width,
            height,
            app.focus_area == FocusArea::FeatureList,
            theme,
            &colors,
            &layout,
        );
    }

    render_status(canvas, text_renderer, app, &colors, &layout, height, width);
}

fn calculate_list_pane_height(app: &App, height: u32) -> u32 {
    if app.task_list.visible || app.error_list.visible {
        (height as f32 * 0.20).round() as u32
    } else {
        0
    }
}

#[allow(clippy::too_many_arguments)]
fn render_split_panes(
    canvas: &mut Canvas,
    text_renderer: &mut TextRenderer,
    app: &App,
    theme: &Theme,
    colors: &render::RenderColors,
    layout: &render::RenderLayout,
    width: u32,
    height: u32,
) {
    let bounds = Rect::new(0, 0, width, height);
    app.splits.render(bounds, |_leaf_id, pane_rect, browser, is_focused| {
        let focused = is_focused && app.focus_area == FocusArea::Splits;
        render::render_browser_pane(
            canvas,
            text_renderer,
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
            colors,
            layout,
            app.icons_enabled,
        );
    });
}

#[allow(clippy::too_many_arguments)]
fn render_task_pane(
    canvas: &mut Canvas,
    text_renderer: &mut TextRenderer,
    app: &App,
    colors: &render::RenderColors,
    layout: &render::RenderLayout,
    y: u32,
    width: u32,
    height: u32,
) {
    let (jobs, cursor, title, empty_msg) = prepare_task_pane_data(app);
    render::render_task_pane(
        canvas,
        text_renderer,
        &jobs,
        cursor,
        title,
        empty_msg,
        0,
        y as i32,
        width,
        height,
        app.focus_area == FocusArea::TaskList,
        colors,
        layout,
    );
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
            all_jobs.iter().filter(|j| j.is_active() || j.is_complete()).collect(),
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

fn render_status(
    canvas: &mut Canvas,
    text_renderer: &mut TextRenderer,
    app: &App,
    colors: &render::RenderColors,
    layout: &render::RenderLayout,
    height: u32,
    width: u32,
) {
    let cursor_info = app.browser().map(|b| (b.cursor, b.entries.len()));
    render::render_status_bar(
        canvas,
        text_renderer,
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
        (height - layout.status_height as u32) as i32,
        width,
        colors,
        layout,
    );
}
