//! Action handlers extracted from App::execute()
//!
//! Each handler returns bool indicating if redraw is needed.

use std::path::PathBuf;

use crate::FocusArea;
use crate::config::Openers;
use crate::filesystem;
use crate::input::{Mode, SortMode};
use crate::jobs;
use crate::navigation::{Browser, Clipboard, Selection};

/// Cursor movement handler
pub fn handle_move_cursor(
    focus_area: FocusArea,
    feature_count: usize,
    feature_pane: &mut crate::features::FeatureListPane,
    task_list: &mut jobs::TaskListPane,
    error_list: &mut jobs::ErrorListPane,
    job_count: usize,
    browser: Option<&mut Browser>,
    selection: &mut Selection,
    mode: Mode,
    delta: i32,
) -> bool {
    if focus_area == FocusArea::FeatureList {
        feature_pane.move_cursor(delta, feature_count);
    } else if focus_area == FocusArea::TaskList {
        if job_count > 0 {
            let cursor_ref = if error_list.visible && !task_list.visible {
                &mut error_list.cursor
            } else {
                &mut task_list.cursor
            };
            if delta > 0 {
                *cursor_ref = (*cursor_ref + delta as usize).min(job_count - 1);
            } else if delta < 0 {
                *cursor_ref = cursor_ref.saturating_sub((-delta) as usize);
            }
        }
    } else if let Some(browser) = browser {
        browser.move_cursor(delta);
        if mode == Mode::Visual {
            selection.add(browser.cursor);
        }
    }
    true
}

/// Open files with configured opener
pub fn open_files(paths: &[PathBuf], openers: &Openers) {
    openers.open_files(paths);
}

/// Yank (copy) files to clipboard
pub fn yank_files(
    browser: Option<&Browser>,
    selection: &Selection,
    mode: Mode,
    clipboard: &mut Clipboard,
) {
    let Some(browser) = browser else { return };

    // Handle archive yanking
    if let Some(archive_path) = browser.get_archive_path() {
        let file_paths: Vec<String> = if mode == Mode::Visual {
            selection
                .to_paths(&browser.entries)
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect()
        } else {
            browser
                .current_entry()
                .map(|e| e.path.to_string_lossy().to_string())
                .into_iter()
                .collect()
        };
        clipboard.yank_from_archive(archive_path.to_path_buf(), file_paths);
    } else {
        let paths = if mode == Mode::Visual {
            selection.to_paths(&browser.entries)
        } else {
            browser
                .current_entry()
                .map(|e| e.path.clone())
                .into_iter()
                .collect()
        };
        clipboard.yank(paths);
    }
}

/// Cut files to clipboard
pub fn cut_files(
    browser: Option<&Browser>,
    selection: &Selection,
    mode: Mode,
    clipboard: &mut Clipboard,
) {
    let paths = if mode == Mode::Visual {
        if let Some(browser) = browser {
            selection.to_paths(&browser.entries)
        } else {
            Vec::new()
        }
    } else if let Some(browser) = browser {
        browser
            .current_entry()
            .map(|e| e.path.clone())
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };

    if !paths.is_empty() {
        clipboard.cut(paths);
    }
}

/// Paste from clipboard
pub fn paste_files(
    browser: Option<&Browser>,
    clipboard: &mut Clipboard,
    job_queue: &mut jobs::JobQueue,
    runtime: &tokio::runtime::Handle,
) {
    let Some(browser) = browser else { return };
    let dest_dir = browser.path.clone();

    if clipboard.is_from_archive() {
        let _ = clipboard.paste_to(&dest_dir);
    } else {
        for src in &clipboard.paths {
            let Some(name) = src.file_name() else {
                continue;
            };
            let dest = dest_dir.join(name);

            let kind = if clipboard.is_cut {
                jobs::JobKind::Move {
                    src: src.clone(),
                    dest,
                }
            } else {
                jobs::JobKind::Copy {
                    src: src.clone(),
                    dest,
                }
            };

            let job_id = job_queue.submit(kind.clone());
            let tx = job_queue.sender();
            runtime.spawn(jobs::execute_job(job_id, kind, tx));
        }

        if clipboard.is_cut {
            clipboard.paths.clear();
            clipboard.is_cut = false;
        }
    }
}

/// Trash files
pub fn trash_files(
    browser: Option<&Browser>,
    selection: &Selection,
    mode: Mode,
    job_queue: &mut jobs::JobQueue,
    runtime: &tokio::runtime::Handle,
) {
    let paths = if mode == Mode::Visual {
        if let Some(browser) = browser {
            selection.to_paths(&browser.entries)
        } else {
            Vec::new()
        }
    } else if let Some(browser) = browser {
        browser
            .current_entry()
            .map(|e| e.path.clone())
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };

    for path in paths {
        let kind = jobs::JobKind::Trash { path };
        let job_id = job_queue.submit(kind.clone());
        let tx = job_queue.sender();
        runtime.spawn(jobs::execute_job(job_id, kind, tx));
    }
}

/// Extract archive
pub fn extract_archive(
    browser: Option<&Browser>,
    job_queue: &mut jobs::JobQueue,
    runtime: &tokio::runtime::Handle,
) {
    let Some(browser) = browser else { return };
    let Some(entry) = browser.current_entry() else {
        return;
    };

    if !entry.is_dir && filesystem::is_archive(&entry.path) {
        let archive = entry.path.clone();
        let dest = browser.path.clone();
        let kind = jobs::JobKind::Extract { archive, dest };
        let job_id = job_queue.submit(kind.clone());
        let tx = job_queue.sender();
        runtime.spawn(jobs::execute_job(job_id, kind, tx));
    }
}

/// Search helpers
pub fn compute_search_matches(browser: Option<&Browser>, pattern: &Option<String>) -> Vec<usize> {
    let Some(browser) = browser else {
        return Vec::new();
    };
    let Some(pattern) = pattern else {
        return Vec::new();
    };

    let pattern_lower = pattern.to_lowercase();
    browser
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.name.to_lowercase().contains(&pattern_lower))
        .map(|(i, _)| i)
        .collect()
}

pub fn find_next_match(matches: &[usize], cursor: usize) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    // Find first match at or after cursor, wrap if needed
    matches.iter().position(|&m| m >= cursor).or(Some(0))
}

pub fn find_prev_match(matches: &[usize], current: Option<usize>) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }

    match current {
        Some(c) if c > 0 => Some(c - 1),
        _ => Some(matches.len() - 1), // Wrap to end
    }
}

/// Task list navigation
pub fn next_task(
    task_list: &mut jobs::TaskListPane,
    error_list: &mut jobs::ErrorListPane,
    job_count: usize,
    focus_area: &mut FocusArea,
) {
    if job_count > 0 {
        if !task_list.visible && !error_list.visible {
            task_list.visible = true;
        }
        *focus_area = FocusArea::TaskList;
        let cursor = if error_list.visible && !task_list.visible {
            &mut error_list.cursor
        } else {
            &mut task_list.cursor
        };
        *cursor = (*cursor + 1).min(job_count - 1);
    }
}

pub fn prev_task(
    task_list: &mut jobs::TaskListPane,
    error_list: &mut jobs::ErrorListPane,
    focus_area: &mut FocusArea,
) {
    if !task_list.visible && !error_list.visible {
        task_list.visible = true;
    }
    *focus_area = FocusArea::TaskList;
    let cursor = if error_list.visible && !task_list.visible {
        &mut error_list.cursor
    } else {
        &mut task_list.cursor
    };
    *cursor = cursor.saturating_sub(1);
}

/// Sorting helpers
pub fn cycle_sort(sort_mode: &mut SortMode) {
    *sort_mode = match sort_mode {
        SortMode::Name => SortMode::Size,
        SortMode::Size => SortMode::Date,
        SortMode::Date => SortMode::Type,
        SortMode::Type => SortMode::Name,
    };
}
