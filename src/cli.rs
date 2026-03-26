//! Command-line argument parsing

use std::collections::HashMap;
use std::path::PathBuf;

use crate::split::SplitDirection;

/// A start path with optional files to select
#[derive(Debug, Clone)]
pub struct StartPath {
    pub directory: PathBuf,
    pub select_files: Vec<String>,
}

pub fn parse_args() -> (Vec<StartPath>, SplitDirection) {
    let mut raw_paths = Vec::new();
    let mut direction = SplitDirection::Vertical;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--vertical" => direction = SplitDirection::Vertical,
            "-s" | "--horizontal" => direction = SplitDirection::Horizontal,
            "-h" | "--help" => print_help(),
            path => raw_paths.push(PathBuf::from(path)),
        }
        i += 1;
    }

    let start_paths = process_paths(raw_paths);
    (start_paths, direction)
}

/// Process raw paths into StartPaths, grouping files by parent directory
fn process_paths(raw_paths: Vec<PathBuf>) -> Vec<StartPath> {
    if raw_paths.is_empty() {
        return Vec::new();
    }

    // Group paths by their effective directory
    let mut dir_files: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let mut dir_order: Vec<PathBuf> = Vec::new();

    for path in raw_paths {
        let canonical = path.canonicalize().unwrap_or(path);

        if canonical.is_dir() {
            dir_files.entry(canonical.clone()).or_insert_with(|| {
                dir_order.push(canonical);
                Vec::new()
            });
        } else if canonical.is_file()
            && let Some(parent) = canonical.parent()
        {
            let parent = parent.to_path_buf();
            let filename = canonical
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let files = dir_files.entry(parent.clone()).or_insert_with(|| {
                dir_order.push(parent);
                Vec::new()
            });
            if !files.contains(&filename) {
                files.push(filename);
            }
        }
    }

    dir_order
        .into_iter()
        .map(|dir| StartPath {
            select_files: dir_files.remove(&dir).unwrap_or_default(),
            directory: dir,
        })
        .collect()
}

fn print_help() -> ! {
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
