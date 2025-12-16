#![allow(dead_code)]

mod actions;
mod app;
mod config;
mod features;
mod filesystem;
mod input;
mod jobs;
mod navigation;
mod preview;
mod render;

use std::path::PathBuf;

use app::{App, FocusArea};
use config::{OverlayPosition, Theme};
use preview::{PreviewCache, is_image_file, is_text_file, render_preview};

use mkframe::{
    App as MkApp, AttachedAnchor, AttachedSurfaceId, Canvas, KeyState, Rect, SplitDirection,
    SubsurfaceId, TextRenderer,
};

fn render(canvas: &mut Canvas, text_renderer: &mut TextRenderer, app: &App, theme: &Theme) {
    let width = canvas.width();
    let height = canvas.height();

    let colors = render::RenderColors::from_theme(theme);
    let layout = render::RenderLayout::default();

    canvas.clear(colors.bg);

    // Calculate task/error list pane height
    let list_pane_visible = app.task_list.visible || app.error_list.visible;
    let list_pane_height = if list_pane_visible {
        (height as f32 * 0.20).round() as u32
    } else {
        0
    };

    // Main content height (minus status bar and list pane)
    let main_content_height = height
        .saturating_sub(layout.status_height as u32)
        .saturating_sub(list_pane_height);

    // Render each split pane
    let bounds = Rect::new(0, 0, width, main_content_height);
    app.splits
        .render(bounds, |_leaf_id, pane_rect, browser, is_focused| {
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
                &colors,
                &layout,
            );
        });

    // Render task/error list pane if visible
    if list_pane_visible {
        let (jobs, cursor, title, empty_msg) = prepare_task_pane_data(app);
        render::render_task_pane(
            canvas,
            text_renderer,
            &jobs,
            cursor,
            title,
            empty_msg,
            0,
            main_content_height as i32,
            width,
            list_pane_height,
            app.focus_area == FocusArea::TaskList,
            &colors,
            &layout,
        );
    }

    // Render feature list panel as overlay
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

    // Render status bar
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
        &colors,
        &layout,
    );
}

fn prepare_task_pane_data(app: &App) -> (Vec<&jobs::Job>, usize, &'static str, &'static str) {
    let all_jobs = app.job_queue.all_jobs();
    let (jobs, cursor, title, empty): (Vec<_>, _, _, _) =
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
        };
    (jobs, cursor, title, empty)
}

fn parse_args() -> (Vec<PathBuf>, SplitDirection) {
    let mut paths = Vec::new();
    let mut direction = SplitDirection::Vertical; // Default to vertical splits

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--vertical" => {
                direction = SplitDirection::Vertical;
            }
            "-s" | "--horizontal" => {
                direction = SplitDirection::Horizontal;
            }
            "-h" | "--help" => {
                eprintln!("Usage: mkfm [OPTIONS] [PATHS...]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  -v, --vertical     Split panes vertically (side-by-side) [default]");
                eprintln!("  -s, --horizontal   Split panes horizontally (stacked)");
                eprintln!("  -h, --help         Show this help message");
                eprintln!();
                eprintln!("Keybindings:");
                eprintln!("  j/k               Move cursor down/up");
                eprintln!("  h/l               Parent/enter directory");
                eprintln!("  gg/G              Go to top/bottom");
                eprintln!("  v                 Enter visual mode");
                eprintln!("  yy                Yank selected");
                eprintln!("  d                 Cut selected");
                eprintln!("  p                 Paste");
                eprintln!("  =                 Open file with default app");
                eprintln!("  :q                Quit");
                eprintln!();
                eprintln!("Split commands (Ctrl+w prefix):");
                eprintln!("  Ctrl+w v          Create vertical split");
                eprintln!("  Ctrl+w s          Create horizontal split");
                eprintln!("  Ctrl+w h/j/k/l    Focus left/down/up/right pane");
                eprintln!("  Ctrl+w c/q        Close current split");
                eprintln!();
                eprintln!("Settings (:set command):");
                eprintln!("  :set hidden       Show hidden files");
                eprintln!("  :set nohidden     Hide hidden files");
                eprintln!("  :set overlay      Enable preview overlay");
                eprintln!("  :set nooverlay    Disable preview overlay");
                eprintln!("  :set parent       Show parent directory entry (..)");
                eprintln!("  :set noparent     Hide parent directory entry");
                eprintln!("  :set theme=NAME   Change theme (e.g., :set theme=dracula)");
                eprintln!("  :set theme=       Reset to default theme");
                std::process::exit(0);
            }
            path => {
                paths.push(PathBuf::from(path));
            }
        }
        i += 1;
    }

    (paths, direction)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (start_paths, split_direction) = parse_args();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    let mut app = rt.block_on(App::new(start_paths, split_direction));
    let decorations = rt.block_on(app.config.decorations());
    let overlay_config = rt.block_on(app.config.overlay());
    let mut text_renderer = TextRenderer::new();

    let (mut mkapp, mut event_queue) = MkApp::new()?;
    let qh = event_queue.handle();

    // Initialize feature availability based on mkframe capabilities
    app.init_features(
        mkapp.has_data_device(),
        mkapp.has_seat(),
        mkapp.has_attached_surface(),
    );

    let window_id = mkapp.create_window_full(&qh, "mkfm", Some("mkfm"), 800, 600, decorations);
    let mut needs_redraw = true;
    let mut preview_path: Option<PathBuf> = None;
    let mut preview_needs_render = false;
    let mut preview_cache = PreviewCache::new();

    // Use attached surface if available (extends beyond window), fall back to subsurface
    let use_attached_surface = mkapp.has_attached_surface();
    let mut preview_attached: Option<AttachedSurfaceId> = None;
    let mut preview_subsurface: Option<SubsurfaceId> = None;

    while mkapp.running {
        // Flush any pending outgoing messages
        mkapp.flush();

        // Poll with timeout based on key repeat state
        let timeout_ms = mkapp.key_repeat_timeout().map(|t| t as i32).unwrap_or(-1);
        let fd = mkapp.connection_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe {
            libc::poll(&mut pfd, 1, timeout_ms);
        }

        // Read any incoming events and dispatch
        if let Some(guard) = event_queue.prepare_read() {
            let _ = guard.read();
        }
        event_queue.dispatch_pending(&mut mkapp)?;

        // Handle keyboard input
        for event in mkapp.poll_key_events() {
            if event.state != KeyState::Pressed {
                continue;
            }

            if let Some(key_str) = event.to_key_string()
                && app.process_key(&key_str)
            {
                needs_redraw = true;
            }
        }

        // Handle pointer input (mouse clicks, scroll)
        let (win_w, win_h) = mkapp.window_size(window_id).unwrap_or((800, 600));
        let modifiers = mkapp.modifiers();
        let ctrl_held = modifiers.ctrl;
        for event in mkapp.poll_pointer_events() {
            if app.handle_pointer_event(&event, win_w, win_h, ctrl_held) {
                needs_redraw = true;
            }
        }

        // Check if we should start a drag operation
        if let Some(files) = app.take_drag_files() {
            let _ = mkapp.start_drag(&qh, window_id, &files);
        }

        // Handle drop events (files dropped from other applications)
        for drop_event in mkapp.poll_drop_events() {
            if !drop_event.files.is_empty() {
                // Copy dropped files to current directory
                if let Some(browser) = app.browser() {
                    let dest_dir = browser.path.clone();
                    for file in drop_event.files {
                        if let Some(file_name) = file.file_name() {
                            let dest = dest_dir.join(file_name);
                            let kind = jobs::JobKind::Copy { src: file, dest };
                            let job_id = app.job_queue.submit(kind.clone());
                            app.runtime.spawn(jobs::execute_job(
                                job_id,
                                kind,
                                app.job_queue.sender(),
                            ));
                        }
                    }
                }
                // Refresh after starting copy jobs
                if let Some(browser) = app.browser_mut() {
                    browser.refresh();
                }
                needs_redraw = true;
            }
        }

        // Poll for job updates (non-blocking)
        let had_active_jobs = app.job_queue.has_active_jobs();
        app.job_queue.poll_updates();

        // If jobs completed, refresh the current browser and trigger redraw
        if had_active_jobs && !app.job_queue.has_active_jobs() {
            if let Some(browser) = app.browser_mut() {
                browser.refresh();
            }
            needs_redraw = true;
        }

        // Also redraw if any jobs are still active (to update progress)
        if app.job_queue.has_active_jobs() {
            needs_redraw = true;
        }

        // Clear completed jobs older than 5 seconds
        app.job_queue.clear_completed(5);

        // Handle exit request
        if app.should_exit {
            break;
        }

        // Handle pending theme change
        if let Some(new_theme_name) = app.pending_theme.take() {
            app.theme = rt.block_on(Theme::load(new_theme_name.as_deref()));
            app.theme_name = new_theme_name;
            needs_redraw = true;
        }

        // Determine if we should show preview (only for previewable files)
        let current_preview_file = app.current_previewable_path();
        let should_show_preview = app.overlay_enabled && current_preview_file.is_some();

        // Get window dimensions for preview sizing
        let (win_w, win_h) = mkapp.window_size(window_id).unwrap_or((800, 600));

        // Calculate preview dimensions
        let preview_width = overlay_config.max_width.resolve(win_w) as u32;
        let preview_height = overlay_config.max_height.resolve(win_h) as u32;

        // Convert config position to anchor
        let anchor = match overlay_config.position {
            OverlayPosition::Right => AttachedAnchor::Right,
            OverlayPosition::Left => AttachedAnchor::Left,
            OverlayPosition::Top => AttachedAnchor::Top,
            OverlayPosition::Bottom => AttachedAnchor::Bottom,
        };
        let margin = overlay_config.margin.resolve(win_w.min(win_h));
        let offset = overlay_config.offset.resolve(
            if matches!(
                overlay_config.position,
                OverlayPosition::Left | OverlayPosition::Right
            ) {
                win_h
            } else {
                win_w
            },
        );

        // Update preview surface state (attached surface preferred, subsurface fallback)
        if should_show_preview {
            let have_preview = preview_attached.is_some() || preview_subsurface.is_some();
            let file_changed = preview_path != current_preview_file;

            if !have_preview || file_changed {
                // Load content first to get actual dimensions
                if let Some(ref path) = current_preview_file {
                    let content = preview_cache.get_or_load(path, preview_width, preview_height);
                    let (actual_w, actual_h) = content.dimensions(preview_width, preview_height);

                    // Close old surface if file changed
                    if file_changed {
                        if let Some(attached_id) = preview_attached.take() {
                            mkapp.close_attached_surface(attached_id);
                        }
                        if let Some(subsurface_id) = preview_subsurface.take() {
                            mkapp.close_subsurface(subsurface_id);
                        }
                    }

                    preview_path = current_preview_file.clone();

                    if preview_attached.is_none() && preview_subsurface.is_none() {
                        if use_attached_surface {
                            preview_attached = mkapp.create_attached_surface(
                                &qh, window_id, 0, // Position handled by anchor
                                0, actual_w, actual_h,
                            );
                            // Set anchor for automatic positioning
                            if let Some(attached_id) = preview_attached {
                                mkapp.set_attached_surface_anchor(
                                    attached_id,
                                    anchor,
                                    margin,
                                    offset,
                                );
                            }
                        } else {
                            // Fallback: calculate position manually for subsurface
                            let (preview_x, preview_y) = match overlay_config.position {
                                OverlayPosition::Right => (win_w as i32 + margin, offset),
                                OverlayPosition::Left => (-(actual_w as i32) - margin, offset),
                                OverlayPosition::Top => (offset, -(actual_h as i32) - margin),
                                OverlayPosition::Bottom => (offset, win_h as i32 + margin),
                            };
                            preview_subsurface = mkapp.create_subsurface(
                                &qh, window_id, preview_x, preview_y, actual_w, actual_h,
                            );
                        }
                    }
                    preview_needs_render = true;
                }
            }
            // Note: No position update needed on resize for attached surfaces - anchor handles it
        } else {
            // Close any open preview surface
            if let Some(attached_id) = preview_attached.take() {
                mkapp.close_attached_surface(attached_id);
                preview_path = None;
                preview_needs_render = false;
                preview_cache.invalidate();
            }
            if let Some(subsurface_id) = preview_subsurface.take() {
                mkapp.close_subsurface(subsurface_id);
                preview_path = None;
                preview_needs_render = false;
                preview_cache.invalidate();
            }
        }

        // Render main window when needed
        if mkapp.is_window_dirty(window_id) || needs_redraw {
            mkapp.render_window(window_id, |canvas| {
                render(canvas, &mut text_renderer, &app, &app.theme);
            });
            mkapp.flush();
        }

        // Render preview surface if open (attached or subsurface)
        if let Some(path) = &preview_path {
            if let Some(attached_id) = preview_attached {
                if mkapp.is_attached_surface_dirty(attached_id) || preview_needs_render {
                    let content =
                        preview_cache.get_or_load(path, preview_width - 20, preview_height - 20);
                    mkapp.render_attached_surface(attached_id, |canvas| {
                        render_preview(canvas, &mut text_renderer, content);
                    });
                    mkapp.flush();
                    preview_needs_render = false;
                }
            } else if let Some(subsurface_id) = preview_subsurface
                && (mkapp.is_subsurface_dirty(subsurface_id) || preview_needs_render)
            {
                let content =
                    preview_cache.get_or_load(path, preview_width - 20, preview_height - 20);
                mkapp.render_subsurface(subsurface_id, |canvas| {
                    render_preview(canvas, &mut text_renderer, content);
                });
                mkapp.flush();
                preview_needs_render = false;
            }
        }

        needs_redraw = false;
    }

    Ok(())
}
