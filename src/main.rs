#![allow(dead_code)]

mod app;
mod app_render;
mod cli;
mod config;
mod event_loop;
mod features;
mod ffmpeg;
mod filesystem;
mod input;
mod jobs;
mod navigation;
mod preview;
mod preview_state;
mod render;

use app::App;
use config::Theme;
use preview::render_preview;
use preview_state::PreviewState;

use mkframe::{App as MkApp, EventQueue, KeyState, QueueHandle, TextRenderer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (start_paths, split_direction) = cli::parse_args();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    let mut app = rt.block_on(App::new(start_paths, split_direction));
    let decorations = rt.block_on(app.config.decorations());
    let overlay_config = rt.block_on(app.config.overlay());
    let mut text_renderer = TextRenderer::new();

    // Check if icon glyphs are available, disable icons if not
    if app.icons_enabled {
        let icon_char = app.theme.icon_folder.chars().next().unwrap_or('\u{f07b}');
        if !text_renderer.has_glyph(icon_char) {
            app.icons_enabled = false;
        }
    }

    let (mut mkapp, mut event_queue) = MkApp::new()?;
    let qh = event_queue.handle();

    app.init_features(
        mkapp.has_data_device(),
        mkapp.has_seat(),
        mkapp.has_attached_surface(),
    );

    let window_id = mkapp.create_window_full(&qh, "mkfm", Some("mkfm"), 800, 600, decorations);
    let mut needs_redraw = true;
    let mut preview = PreviewState::new(mkapp.has_attached_surface());

    while mkapp.running {
        mkapp.flush();
        poll_wayland_events(&mkapp);
        dispatch_events(&mut event_queue, &mut mkapp)?;

        needs_redraw |= handle_input(&mut app, &mut mkapp, window_id);
        needs_redraw |= handle_events(&mut app, &mut mkapp, &qh, window_id);

        if app.should_exit {
            break;
        }

        handle_theme_change(&mut app, &rt, &mut needs_redraw);

        let (win_w, win_h) = mkapp.window_size(window_id).unwrap_or((800, 600));
        preview.update(
            &app,
            &mut mkapp,
            &qh,
            window_id,
            &overlay_config,
            win_w,
            win_h,
        );

        render_if_needed(
            &mut mkapp,
            window_id,
            &mut text_renderer,
            &app,
            &mut needs_redraw,
        );
        render_preview_if_needed(
            &mut mkapp,
            &mut text_renderer,
            &mut preview,
            &overlay_config,
            win_w,
            win_h,
        );
    }

    Ok(())
}

fn poll_wayland_events(mkapp: &MkApp) {
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
}

fn dispatch_events(
    event_queue: &mut EventQueue<MkApp>,
    mkapp: &mut MkApp,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(guard) = event_queue.prepare_read() {
        let _ = guard.read();
    }
    event_queue.dispatch_pending(mkapp)?;
    Ok(())
}

fn handle_input(app: &mut App, mkapp: &mut MkApp, window_id: mkframe::WindowId) -> bool {
    let mut needs_redraw = false;

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

    let (win_w, win_h) = mkapp.window_size(window_id).unwrap_or((800, 600));
    let ctrl_held = mkapp.modifiers().ctrl;
    for event in mkapp.poll_pointer_events() {
        if app.handle_pointer_event(&event, win_w, win_h, ctrl_held) {
            needs_redraw = true;
        }
    }

    needs_redraw
}

fn handle_events(
    app: &mut App,
    mkapp: &mut MkApp,
    qh: &QueueHandle<MkApp>,
    window_id: mkframe::WindowId,
) -> bool {
    let mut needs_redraw = false;

    if let Some(files) = app.take_drag_files() {
        let _ = mkapp.start_drag(qh, window_id, &files);
    }

    if event_loop::handle_drop_events(app, mkapp.poll_drop_events().into_iter()) {
        needs_redraw = true;
    }

    if event_loop::poll_job_updates(app) {
        needs_redraw = true;
    }

    needs_redraw
}

fn handle_theme_change(app: &mut App, rt: &tokio::runtime::Runtime, needs_redraw: &mut bool) {
    if let Some(new_theme_name) = app.pending_theme.take() {
        app.theme = rt.block_on(Theme::load(new_theme_name.as_deref()));
        app.theme_name = new_theme_name;
        *needs_redraw = true;
    }
}

fn render_if_needed(
    mkapp: &mut MkApp,
    window_id: mkframe::WindowId,
    text_renderer: &mut TextRenderer,
    app: &App,
    needs_redraw: &mut bool,
) {
    if mkapp.is_window_dirty(window_id) || *needs_redraw {
        mkapp.render_window(window_id, |canvas| {
            app_render::render_app(canvas, text_renderer, app, &app.theme);
        });
        mkapp.flush();
        *needs_redraw = false;
    }
}

fn render_preview_if_needed(
    mkapp: &mut MkApp,
    text_renderer: &mut TextRenderer,
    preview: &mut PreviewState,
    overlay_config: &config::OverlayConfig,
    win_w: u32,
    win_h: u32,
) {
    let Some(ref path) = preview.path else { return };
    let preview_width = overlay_config.max_width.resolve(win_w) as u32;
    let preview_height = overlay_config.max_height.resolve(win_h) as u32;

    if let Some(attached_id) = preview.attached {
        if mkapp.is_attached_surface_dirty(attached_id) || preview.needs_render {
            let content = preview
                .cache
                .get_or_load(path, preview_width, preview_height);
            mkapp.render_attached_surface(attached_id, |canvas| {
                render_preview(canvas, text_renderer, content);
            });
            mkapp.flush();
            preview.needs_render = false;
        }
    } else if let Some(subsurface_id) = preview.subsurface
        && (mkapp.is_subsurface_dirty(subsurface_id) || preview.needs_render)
    {
        let content = preview
            .cache
            .get_or_load(path, preview_width, preview_height);
        mkapp.render_subsurface(subsurface_id, |canvas| {
            render_preview(canvas, text_renderer, content);
        });
        mkapp.flush();
        preview.needs_render = false;
    }
}
