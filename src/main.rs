#![allow(dead_code)]

mod app;
mod app_render;
mod attached_surface;
mod cli;
mod config;
mod event_loop;
mod features;
mod ffmpeg;
mod filesystem;
mod input;
mod jobs;
mod navigation;
mod overlay;
mod preview;
mod preview_state;
mod render;

use app::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (start_paths, split_direction) = cli::parse_args();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    let mut app = rt.block_on(App::new(start_paths, split_direction));

    let args: Vec<String> = std::env::args().collect();
    let gui_mode = args.iter().any(|a| a == "--gui");

    if gui_mode {
        run_gui(&mut app)
    } else {
        run_tui(&mut app)
    }
}

fn run_tui(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    use mkui::event::EventPoller;
    use mkui::render::Renderer;
    use mkui::tui::TerminalRenderer;
    use std::time::Duration;

    let mut renderer = TerminalRenderer::new()?;
    renderer.enter_alt_screen()?;
    let mut preview = preview_state::PreviewState::new_inline();
    let mut tracker = mkui::component_state::RenderTracker::new();

    let events = EventPoller::new()?;

    loop {
        // Render — tracker decides what actually gets painted
        renderer.begin_frame()?;
        app_render::render_app(&mut renderer, app, &app.theme, &mut preview, &mut tracker);
        renderer.end_frame()?;

        // Poll timeout: match video frame duration when playing, idle otherwise
        let poll_timeout = if let Some(ref playback) = app.playback {
            if playback.playing {
                playback.frame_duration
            } else {
                Duration::from_millis(100)
            }
        } else {
            Duration::from_millis(100)
        };
        let event = events.poll(poll_timeout)?;

        if let Some(event) = event {
            let _ = process_event(app, &event, &mut renderer, &mut tracker)?;

            while let Ok(Some(event)) = events.poll(Duration::ZERO) {
                let _ = process_event(app, &event, &mut renderer, &mut tracker)?;
            }
        }

        if app.should_exit {
            break;
        }

        event_loop::poll_job_updates(app);

        // Stop playback if cursor moved to a different file
        if app.playback.is_some() {
            let current_file = app.browser().and_then(|b| {
                b.entries.get(b.cursor).map(|e| b.path.join(&e.name))
            });
            if current_file.as_deref() != app.media_path.as_deref() {
                app.playback.take();
            }
        }

        if let Some(ref mut playback) = app.playback {
            playback.advance();
        }
    }

    Ok(())
}

fn run_gui(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    use mkui::app::App as MkuiApp;
    use mkui::event::{Event, EventKind};
    use mkui::render::Renderer;

    let mut app = std::mem::replace(app, {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(App::new(vec![], mkui::layout::SplitDirection::Vertical))
    });

    let mut preview: Option<preview_state::PreviewState> = None;
    let mut tracker = mkui::component_state::RenderTracker::new();

    MkuiApp::run_gui("mkfm", 16.0, move |event: &Event, renderer: &mut dyn Renderer| {
        if let Some(key) = event.kind.pressed_key() {
            if let Some(key_str) = key_to_string(key, event) {
                app.process_key(&key_str);
            }
        }

        if let EventKind::Mouse(mouse) = &event.kind {
            let (w, h) = renderer.dimensions();
            let ctrl = matches!(&event.kind, EventKind::Mouse(mkui::event::MouseEvent::Button { modifiers, .. }) if modifiers.ctrl);
            app.handle_mouse_event(mouse, w, h, ctrl);
        }

        if let EventKind::Drop(files) = &event.kind {
            event_loop::handle_drop_events(&mut app, files);
        }

        if app.should_exit {
            return false;
        }

        if matches!(event.kind, EventKind::Redraw) {
            // Initialize preview state on first frame — determines mode once
            let preview = preview.get_or_insert_with(|| {
                // Render one frame first so the compositor has a committed surface
                let _ = renderer.begin_frame();
                let _ = renderer.end_frame();
                preview_state::PreviewState::new_for_renderer(renderer)
            });

            // Video playback: stop if cursor moved to a different file
            if app.playback.is_some() {
                let current = app.browser().and_then(|b| {
                    b.entries.get(b.cursor).map(|e| b.path.join(&e.name))
                });
                if current.as_deref() != app.media_path.as_deref() {
                    app.playback.take();
                }
            }
            if let Some(ref mut playback) = app.playback {
                playback.advance();
            }

            event_loop::poll_job_updates(&mut app);

            let current_file = app.browser().and_then(|b| {
                b.entries.get(b.cursor).map(|e| b.path.join(&e.name))
            });
            preview.update_overlay(renderer, current_file.as_deref(), app.preview_enabled);
            preview.render_overlay();

            let _ = renderer.begin_frame();
            let _ = renderer.clear();
            app_render::render_app(renderer, &app, &app.theme, preview, &mut tracker);
            let _ = renderer.end_frame();
        }

        true
    })?;

    Ok(())
}

/// Process a single event, returns true if state changed (needs redraw)
fn process_event(
    app: &mut App,
    event: &mkui::event::Event,
    renderer: &mut mkui::tui::TerminalRenderer,
    tracker: &mut mkui::component_state::RenderTracker,
) -> Result<bool, Box<dyn std::error::Error>> {
    use mkui::event::EventKind;

    let mut changed = false;

    if let EventKind::Resize(_, _) = &event.kind {
        renderer.refresh_geometry()?;
        let _ = mkui::render::Renderer::clear(renderer);
        let _ = mkui::render::Renderer::clear_images(renderer);
        tracker.invalidate_all();
        changed = true;
    }

    if let Some(key) = event.kind.pressed_key() {
        if let Some(key_str) = key_to_string(key, event) {
            changed |= app.process_key(&key_str);
        }
    }

    if let EventKind::Mouse(mouse) = &event.kind {
        let (w, h) = mkui::render::Renderer::dimensions(renderer);
        let ctrl = matches!(&event.kind, EventKind::Mouse(mkui::event::MouseEvent::Button { modifiers, .. }) if modifiers.ctrl);
        changed |= app.handle_mouse_event(mouse, w, h, ctrl);
    }

    if let EventKind::Drop(files) = &event.kind {
        changed |= event_loop::handle_drop_events(app, files);
    }

    Ok(changed)
}

/// Convert mkui Key to the string format mkfm's input system expects
fn key_to_string(key: &mkui::event::Key, event: &mkui::event::Event) -> Option<String> {
    use mkui::event::{EventKind, Key, Modifiers};

    let mods = match &event.kind {
        EventKind::Key { modifiers, .. } => *modifiers,
        _ => Modifiers::none(),
    };

    if mods.ctrl {
        return match key {
            Key::Char(c) => Some(format!("C-{c}")),
            _ => None,
        };
    }

    if mods.alt {
        return match key {
            Key::Char(c) => Some(format!("A-{c}")),
            _ => None,
        };
    }

    match key {
        Key::Char(c) => Some(c.to_string()),
        Key::Enter => Some("\n".to_string()),
        Key::Esc => Some("\u{1b}".to_string()),
        Key::Tab => Some("Tab".to_string()),
        Key::Backspace => Some("\u{8}".to_string()),
        Key::Delete => Some("Delete".to_string()),
        Key::Up => Some("Up".to_string()),
        Key::Down => Some("Down".to_string()),
        Key::Left => Some("Left".to_string()),
        Key::Right => Some("Right".to_string()),
        Key::Home => Some("Home".to_string()),
        Key::End => Some("End".to_string()),
        Key::PageUp => Some("PageUp".to_string()),
        Key::PageDown => Some("PageDown".to_string()),
        Key::Space => Some(" ".to_string()),
        Key::F(n) => Some(format!("F{n}")),
        _ => None,
    }
}
