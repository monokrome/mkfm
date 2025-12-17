//! Event loop helpers

use std::path::Path;

use crate::app::App;
use crate::jobs;

/// Handle files dropped from external applications
pub fn handle_drop_events(
    app: &mut App,
    drop_events: impl Iterator<Item = mkframe::DropEvent>,
) -> bool {
    let mut needs_redraw = false;

    for drop_event in drop_events {
        if drop_event.files.is_empty() {
            continue;
        }

        if let Some(browser) = app.browser() {
            let dest_dir = browser.path.clone();
            for file in drop_event.files {
                submit_copy_job(app, &file, &dest_dir);
            }
        }

        if let Some(browser) = app.browser_mut() {
            browser.refresh();
        }
        needs_redraw = true;
    }

    needs_redraw
}

fn submit_copy_job(app: &mut App, file: &Path, dest_dir: &Path) {
    if let Some(file_name) = file.file_name() {
        let dest = dest_dir.join(file_name);
        let kind = jobs::JobKind::Copy {
            src: file.to_path_buf(),
            dest,
        };
        let job_id = app.job_queue.submit(kind.clone());
        app.runtime
            .spawn(jobs::execute_job(job_id, kind, app.job_queue.sender()));
    }
}

/// Poll and handle job updates
pub fn poll_job_updates(app: &mut App) -> bool {
    let had_active_jobs = app.job_queue.has_active_jobs();
    app.job_queue.poll_updates();

    let mut needs_redraw = false;

    if had_active_jobs && !app.job_queue.has_active_jobs() {
        if let Some(browser) = app.browser_mut() {
            browser.refresh();
        }
        needs_redraw = true;
    }

    if app.job_queue.has_active_jobs() {
        needs_redraw = true;
    }

    app.job_queue.clear_completed(5);

    needs_redraw
}
