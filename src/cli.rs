//! Command-line argument parsing

use std::path::PathBuf;

use mkframe::SplitDirection;

pub fn parse_args() -> (Vec<PathBuf>, SplitDirection) {
    let mut paths = Vec::new();
    let mut direction = SplitDirection::Vertical;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--vertical" => direction = SplitDirection::Vertical,
            "-s" | "--horizontal" => direction = SplitDirection::Horizontal,
            "-h" | "--help" => print_help(),
            path => paths.push(PathBuf::from(path)),
        }
        i += 1;
    }

    (paths, direction)
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
