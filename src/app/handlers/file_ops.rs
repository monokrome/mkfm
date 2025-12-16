//! File operation handlers

use std::path::Path;

use crate::app::App;
use crate::filesystem;
use crate::jobs;

impl App {
    pub fn execute_open_file(&mut self) -> bool {
        let paths = if self.mode == crate::input::Mode::Visual {
            if let Some(browser) = self.browser() {
                self.selection.to_paths(&browser.entries)
            } else {
                Vec::new()
            }
        } else if let Some(browser) = self.browser() {
            browser
                .current_entry()
                .filter(|e| !e.is_dir)
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if !paths.is_empty() {
            self.openers.open_files(&paths);
        }
        false
    }

    pub fn execute_yank(&mut self) -> bool {
        if let Some(browser) = self.browser() {
            if browser.in_archive() {
                if let Some(archive_path) = browser.get_archive_path().map(Path::to_path_buf) {
                    let file_paths: Vec<String> = if self.selection.is_empty() {
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
                    };
                    self.clipboard.yank_from_archive(archive_path, file_paths);
                }
            } else {
                let paths = self.selected_paths();
                self.clipboard.yank(paths);
            }
        }
        self.exit_visual_if_active();
        true
    }

    pub fn execute_cut(&mut self) -> bool {
        let paths = self.selected_paths();
        self.clipboard.cut(paths);
        self.exit_visual_if_active();
        true
    }

    pub fn execute_paste(&mut self) -> bool {
        if let Some(browser) = self.browser() {
            let dest_dir = browser.path.clone();

            if self.clipboard.is_from_archive() {
                let _ = self.clipboard.paste_to(&dest_dir);
            } else {
                for src in &self.clipboard.paths {
                    let Some(name) = src.file_name() else {
                        continue;
                    };
                    let dest = dest_dir.join(name);

                    let kind = if self.clipboard.is_cut {
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

                    let job_id = self.job_queue.submit(kind.clone());
                    let tx = self.job_queue.sender();
                    self.runtime.spawn(jobs::execute_job(job_id, kind, tx));
                }

                if self.clipboard.is_cut {
                    self.clipboard.paths.clear();
                    self.clipboard.is_cut = false;
                }
            }
        }
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
        true
    }

    pub fn execute_delete(&mut self) -> bool {
        if let Some(browser) = self.browser()
            && let Some(entry) = browser.current_entry()
        {
            let _ = filesystem::delete(&entry.path);
        }
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
        true
    }

    pub fn execute_trash(&mut self) -> bool {
        let paths = self.selected_paths();
        for path in paths {
            let kind = jobs::JobKind::Trash { path: path.clone() };
            let job_id = self.job_queue.submit(kind.clone());
            let tx = self.job_queue.sender();
            self.runtime.spawn(jobs::execute_job(job_id, kind, tx));
        }
        self.exit_visual_if_active();
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
        true
    }

    pub fn execute_create_symlink(&mut self) -> bool {
        if let Some(browser) = self.browser() {
            let dest_dir = browser.path.clone();
            for src in &self.clipboard.paths {
                let _ = filesystem::create_symlink(src, &dest_dir);
            }
        }
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
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
            let job_id = self.job_queue.submit(kind.clone());
            let tx = self.job_queue.sender();
            self.runtime.spawn(jobs::execute_job(job_id, kind, tx));
        }
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
        true
    }
}
