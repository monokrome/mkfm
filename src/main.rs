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
mod preview;
mod preview_state;
mod render;

use app::App;
use config::Theme;

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
    use mkui::event::{EventKind, EventPoller, Key};
    use mkui::render::Renderer;
    use mkui::tui::TerminalRenderer;

    let mut renderer = TerminalRenderer::new()?;
    renderer.enter_alt_screen()?;
    let mut preview_cache = preview::PreviewCache::new();

    let events = EventPoller::new()?;

    loop {
        // Render
        renderer.begin_frame()?;
        renderer.clear()?;
        app_render::render_app(&mut renderer, app, &app.theme, &mut preview_cache);
        renderer.end_frame()?;

        // Poll jobs
        if event_loop::poll_job_updates(app) {
            continue;
        }

        // Handle input
        let event = events.read()?;

        if let EventKind::Resize(_, _) = &event.kind {
            renderer.refresh_geometry()?;
            continue;
        }

        if let Some(key) = event.kind.pressed_key() {
            if let Some(key_str) = key_to_string(key, &event) {
                app.process_key(&key_str);
            }
        }

        // Handle mouse events
        if let EventKind::Mouse(mouse) = &event.kind {
            let (w, h) = renderer.dimensions();
            let ctrl = matches!(&event.kind, EventKind::Mouse(mkui::event::MouseEvent::Button { modifiers, .. }) if modifiers.ctrl);
            app.handle_mouse_event(mouse, w, h, ctrl);
        }

        // Handle drop events
        if let EventKind::Drop(files) = &event.kind {
            event_loop::handle_drop_events(app, files);
        }

        if app.should_exit {
            break;
        }
    }

    Ok(())
}

fn run_gui(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    use mkui::app::App as MkuiApp;
    use mkui::event::{Event, EventKind, Key};
    use mkui::render::Renderer;

    // Take ownership of app for the GUI closure
    let mut app = std::mem::replace(app, {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(App::new(vec![], mkui::layout::SplitDirection::Vertical))
    });

    let mut preview_cache = preview::PreviewCache::new();

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
            event_loop::poll_job_updates(&mut app);

            let _ = renderer.begin_frame();
            let _ = renderer.clear();
            app_render::render_app(renderer, &app, &app.theme, &mut preview_cache);
            let _ = renderer.end_frame();
        }

        true
    })?;

    Ok(())
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
