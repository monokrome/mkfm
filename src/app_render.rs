//! Application rendering — composes mkui rendering for the file manager UI

use mkui::component_state::RenderTracker;
use mkui::layout::Rect;
use mkui::render::Renderer;
use mkui::style::Style;

use crate::app::{App, FocusArea};
use crate::config::Theme;
use crate::jobs::Job;
use crate::preview::PreviewCache;
use crate::preview::render_preview;
use crate::preview_state::PreviewState;
use crate::render;

/// Minimum pane width to show inline preview
const PREVIEW_MIN_WIDTH: u16 = 40;

/// Minimum columns for the file list
const MIN_LIST_WIDTH: u16 = 20;

/// Maximum fraction the file list can take
const MAX_LIST_RATIO: f32 = 0.5;

/// Component IDs for the render tracker
const ID_STATUS: usize = 1000;
const ID_TASK_PANE: usize = 1001;

/// Render the entire application UI using incremental tracking
pub fn render_app(
    renderer: &mut dyn Renderer,
    app: &App,
    theme: &Theme,
    preview: &mut PreviewState,
    tracker: &mut RenderTracker,
) {
    let (width, height) = renderer.dimensions();
    let colors = render::RenderColors::from_theme(theme);
    let layout = render::RenderLayout::default();

    let list_pane_height = calculate_list_pane_height(app, height);
    let main_height = height
        .saturating_sub(layout.status_height)
        .saturating_sub(list_pane_height);

    // Draw borders between split panes
    let bounds = Rect::new(0, 0, width, main_height);
    let pane_layout = app.splits.layout(bounds);
    for (_, pane_rect) in &pane_layout {
        if pane_rect.x > 0 {
            let border_style = Style::new().fg(colors.border);
            for row in 0..pane_rect.height {
                let _ = renderer.move_cursor(pane_rect.x.saturating_sub(1), pane_rect.y + row);
                let _ = renderer.write_styled("│", &border_style);
            }
        }
    }

    // Render split panes (file browsers)
    app.splits
        .render(bounds, |leaf_id, pane_rect, browser, is_focused| {
            let focused = is_focused && app.focus_area == FocusArea::Splits;
            let pane_id = leaf_id.0;

            // Generation: hash of path + cursor + entry count
            // Path hash ensures directory changes trigger repaint
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            browser.path.hash(&mut hasher);
            browser.cursor.hash(&mut hasher);
            browser.entries.len().hash(&mut hasher);
            let browser_gen = hasher.finish();

            let show_preview = focused
                && app.overlay_enabled
                && !preview.is_overlay_active()
                && pane_rect.width >= PREVIEW_MIN_WIDTH;

            let (list_rect, preview_rect) = if show_preview {
                let list_w = if let Some(zoom) = app.preview_zoom {
                    // Zoom overrides: preview gets `zoom` fraction, list gets the rest
                    let preview_w = (pane_rect.width as f32 * zoom) as u16;
                    pane_rect.width.saturating_sub(preview_w).max(MIN_LIST_WIDTH)
                } else {
                    calculate_list_width(browser, pane_rect.width)
                };
                let preview_w = pane_rect.width.saturating_sub(list_w);
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

            // Track the full pane — not just the list portion
            if tracker.needs_render(renderer, pane_id, browser_gen, pane_rect) {
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
            }

            // Preview pane — uses same generation as browser so they stay in sync
            if let Some(prev_rect) = preview_rect {
                let preview_id = pane_id + 500;

                if let Some(ref playback) = app.playback {
                    if !playback.current_frame.is_empty() {
                        // Clear preview area before rendering video frame
                        let _ = renderer.fill_rect(prev_rect, colors.bg);
                        let dst = mkui::layout::ObjectFit::Contain.fit_with_aspect(
                            playback.width,
                            playback.height,
                            prev_rect,
                            renderer.cell_aspect(),
                        );
                        let _ = renderer.render_image(&mkui::render::ImageParams {
                            data: &playback.current_frame,
                            width: playback.width,
                            height: playback.height,
                            col: dst.x,
                            row: dst.y,
                            width_cells: Some(dst.width),
                            height_cells: Some(dst.height),
                        });
                    }
                } else {
                    // Static preview — same generation as browser pane
                    if tracker.needs_render(renderer, preview_id, browser_gen, prev_rect) {
                        render_inline_preview(
                            renderer,
                            browser,
                            &mut preview.cache,
                            prev_rect,
                            &colors,
                        );
                    }
                }
            }
        });

    // Task/error pane
    if app.task_list.visible || app.error_list.visible {
        let task_bounds = Rect::new(0, main_height, width, list_pane_height);
        let task_gen = app.job_queue.all_jobs().len() as u64;

        if tracker.needs_render(renderer, ID_TASK_PANE, task_gen, task_bounds) {
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
    }

    // Status bar
    let status_y = height.saturating_sub(layout.status_height);
    let status_bounds = Rect::new(0, status_y, width, layout.status_height);
    // Mode + cursor position as generation
    let status_gen = (app.mode as u64) << 48
        | app.browser().map_or(0, |b| (b.cursor as u64) << 16 | b.entries.len() as u64);

    if tracker.needs_render(renderer, ID_STATUS, status_gen, status_bounds) {
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
}

fn render_inline_preview(
    renderer: &mut dyn Renderer,
    browser: &crate::navigation::Browser,
    cache: &mut PreviewCache,
    bounds: Rect,
    colors: &render::RenderColors,
) {
    if browser.cursor >= browser.entries.len() {
        return;
    }

    let entry = &browser.entries[browser.cursor];
    let file_path = browser.path.join(&entry.name);

    if entry.is_dir {
        let _ = renderer.fill_rect(bounds, colors.bg);
        let _ = renderer.move_cursor(bounds.x + 1, bounds.y + 1);
        let _ = renderer.write_styled(
            "(directory)",
            &Style::new().fg(colors.fg).dim(true),
        );
        return;
    }

    let content = cache.get_or_load(&file_path, 1920, 1080);
    render_preview(renderer, content, bounds, colors.fg, colors.bg);
}

fn calculate_list_width(browser: &crate::navigation::Browser, pane_width: u16) -> u16 {
    let max_name_len = browser
        .entries
        .iter()
        .map(|e| e.name.len())
        .max()
        .unwrap_or(10) as u16;

    let needed = max_name_len + 3;
    let max_list = (pane_width as f32 * MAX_LIST_RATIO) as u16;
    needed.max(MIN_LIST_WIDTH).min(max_list)
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
