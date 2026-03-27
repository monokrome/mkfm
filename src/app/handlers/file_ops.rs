//! File operation handlers

use std::path::{Path, PathBuf};

use crate::app::App;
use crate::filesystem;
use crate::input::Mode;
use crate::jobs;

impl App {
    pub fn execute_open_file(&mut self) -> bool {
        // If current entry is a directory, enter it
        if let Some(entry) = self.browser().and_then(|b| b.current_entry()) {
            if entry.is_dir {
                return self.execute_enter_directory();
            }
        }

        // Otherwise open the file
        let paths = self.get_paths_for_open();
        if !paths.is_empty() {
            if let Some(mime) = self.openers.open_files(&paths) {
                self.status_message = Some(format!("No opener for: {mime}"));
            }
        }
        true
    }

    fn get_paths_for_open(&self) -> Vec<PathBuf> {
        if self.mode == Mode::Visual {
            self.browser()
                .map(|b| self.selection.to_paths(&b.entries))
                .unwrap_or_default()
        } else {
            self.browser()
                .and_then(|b| b.current_entry())
                .filter(|e| !e.is_dir)
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default()
        }
    }

    pub fn execute_yank(&mut self) -> bool {
        let yank_action = self.determine_yank_action();
        match yank_action {
            YankAction::Archive {
                archive_path,
                file_paths,
            } => {
                self.clipboard.yank_from_archive(archive_path, file_paths);
            }
            YankAction::Filesystem { paths } => {
                self.clipboard.yank(paths);
            }
            YankAction::None => {}
        }
        self.exit_visual_if_active();
        true
    }

    fn determine_yank_action(&self) -> YankAction {
        let Some(browser) = self.browser() else {
            return YankAction::None;
        };
        if browser.in_archive() {
            let Some(archive_path) = browser.get_archive_path().map(Path::to_path_buf) else {
                return YankAction::None;
            };
            let file_paths = self.get_archive_file_paths(browser);
            YankAction::Archive {
                archive_path,
                file_paths,
            }
        } else {
            YankAction::Filesystem {
                paths: self.selected_paths(),
            }
        }
    }

    fn get_archive_file_paths(&self, browser: &crate::navigation::Browser) -> Vec<String> {
        if self.selection.is_empty() {
            browser
                .current_entry()
                .map(|e| vec![e.path.to_string_lossy().to_string()])
                .unwrap_or_default()
        } else {
            self.selection
                .to_paths(&browser.entries)
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect()
        }
    }

    pub fn execute_cut(&mut self) -> bool {
        self.clipboard.cut(self.selected_paths());
        self.exit_visual_if_active();
        true
    }

    pub fn execute_paste(&mut self) -> bool {
        let paste_action = self.determine_paste_action();
        match paste_action {
            PasteAction::Archive { dest_dir } => {
                let _ = self.clipboard.paste_to(&dest_dir);
            }
            PasteAction::Filesystem { dest_dir } => {
                self.paste_filesystem_files(&dest_dir);
            }
            PasteAction::None => {}
        }
        self.refresh_browser();
        true
    }

    fn determine_paste_action(&self) -> PasteAction {
        let Some(browser) = self.browser() else {
            return PasteAction::None;
        };
        let dest_dir = browser.path.clone();
        if self.clipboard.is_from_archive() {
            PasteAction::Archive { dest_dir }
        } else {
            PasteAction::Filesystem { dest_dir }
        }
    }

    fn paste_filesystem_files(&mut self, dest_dir: &Path) {
        let jobs: Vec<_> = self
            .clipboard
            .paths
            .iter()
            .filter_map(|src| {
                let name = src.file_name()?;
                Some(make_paste_job_kind(
                    src,
                    dest_dir.join(name),
                    self.clipboard.is_cut,
                ))
            })
            .collect();

        for kind in jobs {
            self.submit_job(kind);
        }

        if self.clipboard.is_cut {
            self.clipboard.paths.clear();
            self.clipboard.is_cut = false;
        }
    }

    pub fn execute_delete(&mut self) -> bool {
        let paths = self.selected_paths();
        if paths.is_empty() {
            if let Some(browser) = self.browser() {
                if let Some(entry) = browser.current_entry() {
                    self.pending_confirm = Some(crate::app::PendingAction::Delete(vec![entry.path.clone()]));
                    return true;
                }
            }
            return false;
        }
        self.pending_confirm = Some(crate::app::PendingAction::Delete(paths));
        true
    }

    pub fn confirm_pending(&mut self) -> bool {
        let Some(action) = self.pending_confirm.take() else {
            return false;
        };
        match action {
            crate::app::PendingAction::Delete(paths) => {
                for path in &paths {
                    let _ = filesystem::delete(path);
                }
                self.refresh_browser();
                true
            }
        }
    }

    pub fn cancel_pending(&mut self) -> bool {
        if self.pending_confirm.is_some() {
            self.pending_confirm = None;
            true
        } else {
            false
        }
    }

    pub fn execute_trash(&mut self) -> bool {
        let jobs: Vec<_> = self
            .selected_paths()
            .into_iter()
            .map(|path| jobs::JobKind::Trash { path })
            .collect();
        for kind in jobs {
            self.submit_job(kind);
        }
        self.exit_visual_if_active();
        self.refresh_browser();
        true
    }

    pub fn execute_create_symlink(&mut self) -> bool {
        if let Some(browser) = self.browser() {
            let dest_dir = browser.path.clone();
            for src in &self.clipboard.paths {
                let _ = filesystem::create_symlink(src, &dest_dir);
            }
        }
        self.refresh_browser();
        true
    }

    pub fn execute_extract_archive(&mut self) -> bool {
        if let Some(browser) = self.browser()
            && let Some(entry) = browser.current_entry()
        {
            let kind = jobs::JobKind::Extract {
                archive: entry.path.clone(),
                dest: browser.path.clone(),
            };
            self.submit_job(kind);
        }
        self.refresh_browser();
        true
    }

    fn submit_job(&mut self, kind: jobs::JobKind) {
        let job_id = self.job_queue.submit(kind.clone());
        let tx = self.job_queue.sender();
        self.runtime.spawn(jobs::execute_job(job_id, kind, tx));
    }

    fn refresh_browser(&mut self) {
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
    }
}

enum YankAction {
    Archive {
        archive_path: PathBuf,
        file_paths: Vec<String>,
    },
    Filesystem {
        paths: Vec<PathBuf>,
    },
    None,
}

enum PasteAction {
    Archive { dest_dir: PathBuf },
    Filesystem { dest_dir: PathBuf },
    None,
}

fn make_paste_job_kind(src: &Path, dest: PathBuf, is_cut: bool) -> jobs::JobKind {
    if is_cut {
        jobs::JobKind::Move {
            src: src.to_path_buf(),
            dest,
        }
    } else {
        jobs::JobKind::Copy {
            src: src.to_path_buf(),
            dest,
        }
    }
}
