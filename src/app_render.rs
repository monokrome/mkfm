//! Application rendering — composes mkui rendering for the file manager UI

use mkui::layout::Rect;
use mkui::render::Renderer;
use mkui::style::Style;

use crate::app::{App, FocusArea};
use crate::config::Theme;
use crate::jobs::Job;
use crate::preview::{self, PreviewCache};
use crate::preview::render_preview;
use crate::preview_state::PreviewState;
use crate::render;

/// Minimum pane width to show inline preview
const PREVIEW_MIN_WIDTH: u16 = 60;

/// Fraction of pane width used for preview (right side)
const PREVIEW_WIDTH_RATIO: f32 = 0.4;

/// Render the entire application UI
pub fn render_app(
    renderer: &mut dyn Renderer,
    app: &App,
    theme: &Theme,
    preview: &mut PreviewState,
) {
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

            // Determine if we should show inline preview
            let show_preview = focused
                && app.overlay_enabled
                && !preview.is_overlay_active()
                && pane_rect.width >= PREVIEW_MIN_WIDTH;

            let (list_rect, preview_rect) = if show_preview {
                let preview_w = (pane_rect.width as f32 * PREVIEW_WIDTH_RATIO) as u16;
                let list_w = pane_rect.width.saturating_sub(preview_w);
                (
                    Rect::new(pane_rect.x, pane_rect.y, list_w, pane_rect.height),
                    Some(Rect::new(
                        pane_rect.x + list_w,
                        pane_rect.y,
                        preview_w,
                        pane_rect.height,
                    )),
                )
            } else {
                (pane_rect, None)
            };

            render::render_browser_pane(
                renderer,
                browser,
                &app.selection,
                app.search_highlight,
                &app.search_matches,
                theme,
                list_rect.x,
                list_rect.y,
                list_rect.width,
                list_rect.height,
                focused,
                &colors,
                &layout,
                app.icons_enabled,
            );

            // Render inline preview if space available
            if let Some(prev_rect) = preview_rect {
                render_inline_preview(
                    renderer,
                    browser,
                    &mut preview.cache,
                    prev_rect,
                    &colors,
                );
            }
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

fn render_inline_preview(
    renderer: &mut dyn Renderer,
    browser: &crate::navigation::Browser,
    cache: &mut PreviewCache,
    bounds: Rect,
    colors: &render::RenderColors,
) {

    // Get current file under cursor
    if browser.cursor >= browser.entries.len() {
        return;
    }

    let entry = &browser.entries[browser.cursor];
    let file_path = browser.path.join(&entry.name);

    if entry.is_dir {
        let _ = renderer.move_cursor(bounds.x + 1, bounds.y + 1);
        let _ = renderer.write_styled(
            "(directory)",
            &Style::new().fg(colors.fg).dim(true),
        );
        return;
    }

    let content = cache.get_or_load(
        &file_path,
        bounds.width as u32 * 10,
        bounds.height as u32 * 20,
    );

    render_preview(renderer, content, bounds, colors.fg, colors.bg);
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
