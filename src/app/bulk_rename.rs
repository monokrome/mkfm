//! Bulk rename functionality

use std::path::{Path, PathBuf};

use super::App;

impl App {
    pub fn execute_bulk_rename(&mut self) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return;
        }

        let temp_path = std::env::temp_dir().join("mkfm_rename.txt");
        let names = collect_file_names(&paths);

        if std::fs::write(&temp_path, names.join("\n")).is_err() {
            return;
        }

        if run_editor(&temp_path).is_ok() {
            apply_renames(&paths, &temp_path);
        }

        let _ = std::fs::remove_file(&temp_path);
        self.exit_visual_if_active();
        if let Some(browser) = self.browser_mut() {
            browser.refresh();
        }
    }
}

fn collect_file_names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .collect()
}

fn run_editor(path: &Path) -> std::io::Result<std::process::ExitStatus> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    std::process::Command::new(&editor).arg(path).status()
}

fn apply_renames(paths: &[PathBuf], temp_path: &Path) {
    if let Ok(content) = std::fs::read_to_string(temp_path) {
        let new_names: Vec<&str> = content.lines().collect();
        for (old_path, new_name) in paths.iter().zip(new_names.iter()) {
            rename_if_valid(old_path, new_name);
        }
    }
}

fn rename_if_valid(old_path: &Path, new_name: &str) {
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return;
    }
    if let Some(parent) = old_path.parent() {
        let new_path = parent.join(new_name);
        if old_path != new_path {
            let _ = std::fs::rename(old_path, &new_path);
        }
    }
}
