#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Visual,
    Command,
    Search,
}

impl Mode {
    pub fn display(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Visual => "VISUAL",
            Mode::Command => "COMMAND",
            Mode::Search => "SEARCH",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum SortMode {
    #[default]
    Name,
    Size,
    Date,
    Type,
}

impl SortMode {
    pub fn display(&self) -> &'static str {
        match self {
            SortMode::Name => "name",
            SortMode::Size => "size",
            SortMode::Date => "date",
            SortMode::Type => "type",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            SortMode::Name => SortMode::Size,
            SortMode::Size => SortMode::Date,
            SortMode::Date => SortMode::Type,
            SortMode::Type => SortMode::Name,
        }
    }
}

#[derive(Clone)]
pub enum Action {
    None,
    Pending,
    MoveCursor(i32),
    CursorToTop,
    CursorToBottom,
    NextDirectory,
    PrevDirectory,
    EnterDirectory,
    ParentDirectory,
    OpenFile,
    EnterVisualMode,
    ExitVisualMode,
    EnterCommandMode,
    CommandAppend(char),
    CommandBackspace,
    CommandExecute,
    CommandCancel,
    Yank,
    Cut,
    Paste,
    Delete,
    Trash,
    ToggleHidden,
    EnableHidden,
    DisableHidden,
    ToggleOverlay,
    EnableOverlay,
    DisableOverlay,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    SplitVertical,
    SplitHorizontal,
    CloseSplit,
    // Search
    EnterSearchMode,
    SearchAppend(char),
    SearchBackspace,
    SearchExecute,
    SearchCancel,
    SearchNext,
    SearchPrev,
    // Bookmarks
    SetMark(char),
    JumpToMark(char),
    // Sorting
    CycleSort,
    ReverseSort,
    // Filter
    ClearFilter,
    // Archives
    ExtractArchive,
    // Symlinks
    CreateSymlink,
    // Search highlighting
    ClearSearchHighlight,
    // Fold (inline expansion)
    FoldOpen,
    FoldClose,
    FoldToggle,
    FoldOpenRecursive,
    FoldCloseRecursive,
    // Task/Error list (quickfix-style)
    NextTask,
    PrevTask,
    ToggleTaskList,
    NextError,
    PrevError,
    ToggleErrorList,
    // Feature list
    ToggleFeatureList,
}

pub fn handle_normal_key(key: &str, pending: &str) -> Action {
    // Handle mark setting: m + any letter
    if pending == "m" {
        if let Some(c) = key.chars().next() {
            if c.is_ascii_alphabetic() {
                return Action::SetMark(c);
            }
        }
        return Action::None;
    }

    // Handle mark jumping: ' + any letter
    if pending == "'" {
        if let Some(c) = key.chars().next() {
            if c.is_ascii_alphabetic() {
                return Action::JumpToMark(c);
            }
        }
        return Action::None;
    }

    match (pending, key) {
        // Unimpaired-style toggles: yo<key>
        ("yo", "o") => Action::ToggleOverlay,
        ("yo", "h") => Action::ToggleHidden,
        ("y", "o") => Action::Pending,

        // Unimpaired-style enable: [o<key>
        ("[o", "o") => Action::EnableOverlay,
        ("[o", "h") => Action::EnableHidden,
        ("[", "o") => Action::Pending,

        // Unimpaired-style disable: ]o<key>
        ("]o", "o") => Action::DisableOverlay,
        ("]o", "h") => Action::DisableHidden,
        ("]", "o") => Action::Pending,

        // Unimpaired-style navigation: [d / ]d for directories
        ("[", "d") => Action::PrevDirectory,
        ("]", "d") => Action::NextDirectory,

        // Task list (quickfix-like): ]q / [q
        ("]", "q") => Action::NextTask,
        ("[", "q") => Action::PrevTask,

        // Error list (location-list-like): ]l / [l
        ("]", "l") => Action::NextError,
        ("[", "l") => Action::PrevError,

        // "yy" yanks
        ("y", "y") => Action::Yank,
        ("", "y") => Action::Pending,

        // Start bracket sequences
        ("", "[") => Action::Pending,
        ("", "]") => Action::Pending,

        // gg goes to top
        ("g", "g") => Action::CursorToTop,
        ("", "g") => Action::Pending,

        // Ctrl+w split commands (vim-style)
        ("C-w", "h") => Action::FocusLeft,
        ("C-w", "j") => Action::FocusDown,
        ("C-w", "k") => Action::FocusUp,
        ("C-w", "l") => Action::FocusRight,
        ("C-w", "v") => Action::SplitVertical,
        ("C-w", "s") => Action::SplitHorizontal,
        ("C-w", "c") | ("C-w", "q") => Action::CloseSplit,
        ("", "C-w") => Action::Pending,

        // Bookmarks
        ("", "m") => Action::Pending,  // Set mark
        ("", "'") => Action::Pending,  // Jump to mark

        // Search
        (_, "/") => Action::EnterSearchMode,
        (_, "n") => Action::SearchNext,
        (_, "N") => Action::SearchPrev,
        (_, "C-l") => Action::ClearSearchHighlight,

        // Sorting
        (_, "s") => Action::CycleSort,
        (_, "S") => Action::ReverseSort,

        // Archives
        (_, "e") => Action::ExtractArchive,

        // Fold (inline expansion) - z-prefix
        ("z", "o") => Action::FoldOpen,
        ("z", "c") => Action::FoldClose,
        ("z", "a") => Action::FoldToggle,
        ("z", "O") => Action::FoldOpenRecursive,
        ("z", "C") => Action::FoldCloseRecursive,
        ("", "z") => Action::Pending,

        // Feature list
        (_, "F12") => Action::ToggleFeatureList,

        // Single key commands
        (_, "j") => Action::MoveCursor(1),
        (_, "k") => Action::MoveCursor(-1),
        (_, "G") => Action::CursorToBottom,
        (_, "l") | (_, "\n") => Action::EnterDirectory,
        (_, "h") | (_, "-") => Action::ParentDirectory,
        (_, "=") => Action::OpenFile,
        (_, "v") => Action::EnterVisualMode,
        (_, "d") => Action::Cut,
        (_, "p") => Action::Paste,
        (_, "x") => Action::Delete,
        (_, "X") => Action::Trash,
        (_, ".") => Action::ToggleHidden,
        (_, ":") => Action::EnterCommandMode,
        (_, "\u{1b}") => Action::ExitVisualMode,
        _ => Action::None,
    }
}

pub fn handle_visual_key(key: &str) -> Action {
    match key {
        "j" => Action::MoveCursor(1),
        "k" => Action::MoveCursor(-1),
        "y" => Action::Yank,
        "d" => Action::Cut,
        "\u{1b}" | "v" => Action::ExitVisualMode,
        _ => Action::None,
    }
}

fn handle_command_key(key: &str) -> Action {
    match key {
        "\u{1b}" => Action::CommandCancel,
        "\n" => Action::CommandExecute,
        "\u{8}" => Action::CommandBackspace,  // Backspace
        _ => {
            let mut chars = key.chars();
            if let Some(c) = chars.next() {
                if chars.next().is_none() && !c.is_control() {
                    return Action::CommandAppend(c);
                }
            }
            Action::None
        }
    }
}

fn handle_search_key(key: &str) -> Action {
    match key {
        "\u{1b}" => Action::SearchCancel,
        "\n" => Action::SearchExecute,
        "\u{8}" => Action::SearchBackspace,
        _ => {
            let mut chars = key.chars();
            if let Some(c) = chars.next() {
                if chars.next().is_none() && !c.is_control() {
                    return Action::SearchAppend(c);
                }
            }
            Action::None
        }
    }
}

/// Handle keyboard input for non-vim (standard) mode
fn handle_normal_key_standard(key: &str) -> Action {
    match key {
        // Arrow key navigation
        "Up" => Action::MoveCursor(-1),
        "Down" => Action::MoveCursor(1),
        "Home" => Action::CursorToTop,
        "End" => Action::CursorToBottom,
        "PageUp" => Action::MoveCursor(-10),
        "PageDown" => Action::MoveCursor(10),

        // Enter and navigation
        "\n" | "Right" => Action::EnterDirectory,
        "Left" | "\u{8}" => Action::ParentDirectory,  // Backspace

        // File operations
        "Delete" => Action::Trash,
        "C-c" => Action::Yank,
        "C-x" => Action::Cut,
        "C-v" => Action::Paste,

        // Search
        "C-f" => Action::EnterSearchMode,
        "F3" => Action::SearchNext,
        "S-F3" => Action::SearchPrev,

        // Misc
        "F5" => Action::ParentDirectory,  // Refresh (re-enter current dir)
        "\u{1b}" => Action::ClearSearchHighlight,  // Escape
        "C-h" => Action::ToggleHidden,
        "F2" => Action::OpenFile,
        "F12" => Action::ToggleFeatureList,

        _ => Action::None,
    }
}

pub fn handle_key(mode: Mode, key: &str, pending: &str, vi_mode: bool) -> Action {
    match mode {
        Mode::Normal => {
            if vi_mode {
                handle_normal_key(key, pending)
            } else {
                handle_normal_key_standard(key)
            }
        }
        Mode::Visual => handle_visual_key(key),
        Mode::Command => handle_command_key(key),
        Mode::Search => handle_search_key(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_display() {
        assert_eq!(Mode::Normal.display(), "NORMAL");
        assert_eq!(Mode::Visual.display(), "VISUAL");
        assert_eq!(Mode::Command.display(), "COMMAND");
        assert_eq!(Mode::Search.display(), "SEARCH");
    }

    #[test]
    fn test_mode_default() {
        assert_eq!(Mode::default(), Mode::Normal);
    }

    #[test]
    fn test_normal_mode_navigation() {
        assert!(matches!(handle_normal_key("j", ""), Action::MoveCursor(1)));
        assert!(matches!(handle_normal_key("k", ""), Action::MoveCursor(-1)));
        assert!(matches!(handle_normal_key("G", ""), Action::CursorToBottom));
        assert!(matches!(handle_normal_key("l", ""), Action::EnterDirectory));
        assert!(matches!(handle_normal_key("h", ""), Action::ParentDirectory));
        assert!(matches!(handle_normal_key("-", ""), Action::ParentDirectory));
    }

    #[test]
    fn test_normal_mode_gg_sequence() {
        // First g should be pending
        assert!(matches!(handle_normal_key("g", ""), Action::Pending));
        // Second g after pending should go to top
        assert!(matches!(handle_normal_key("g", "g"), Action::CursorToTop));
    }

    #[test]
    fn test_normal_mode_yy_sequence() {
        assert!(matches!(handle_normal_key("y", ""), Action::Pending));
        assert!(matches!(handle_normal_key("y", "y"), Action::Yank));
    }

    #[test]
    fn test_normal_mode_unimpaired_toggles() {
        // yo sequence for toggle
        assert!(matches!(handle_normal_key("o", "y"), Action::Pending));
        assert!(matches!(handle_normal_key("o", "yo"), Action::ToggleOverlay));
        assert!(matches!(handle_normal_key("h", "yo"), Action::ToggleHidden));
    }

    #[test]
    fn test_normal_mode_unimpaired_enable() {
        assert!(matches!(handle_normal_key("[", ""), Action::Pending));
        assert!(matches!(handle_normal_key("o", "["), Action::Pending));
        assert!(matches!(handle_normal_key("o", "[o"), Action::EnableOverlay));
        assert!(matches!(handle_normal_key("h", "[o"), Action::EnableHidden));
    }

    #[test]
    fn test_normal_mode_unimpaired_disable() {
        assert!(matches!(handle_normal_key("]", ""), Action::Pending));
        assert!(matches!(handle_normal_key("o", "]"), Action::Pending));
        assert!(matches!(handle_normal_key("o", "]o"), Action::DisableOverlay));
        assert!(matches!(handle_normal_key("h", "]o"), Action::DisableHidden));
    }

    #[test]
    fn test_normal_mode_directory_navigation() {
        assert!(matches!(handle_normal_key("d", "["), Action::PrevDirectory));
        assert!(matches!(handle_normal_key("d", "]"), Action::NextDirectory));
    }

    #[test]
    fn test_normal_mode_split_commands() {
        assert!(matches!(handle_normal_key("C-w", ""), Action::Pending));
        assert!(matches!(handle_normal_key("h", "C-w"), Action::FocusLeft));
        assert!(matches!(handle_normal_key("j", "C-w"), Action::FocusDown));
        assert!(matches!(handle_normal_key("k", "C-w"), Action::FocusUp));
        assert!(matches!(handle_normal_key("l", "C-w"), Action::FocusRight));
        assert!(matches!(handle_normal_key("v", "C-w"), Action::SplitVertical));
        assert!(matches!(handle_normal_key("s", "C-w"), Action::SplitHorizontal));
        assert!(matches!(handle_normal_key("c", "C-w"), Action::CloseSplit));
        assert!(matches!(handle_normal_key("q", "C-w"), Action::CloseSplit));
    }

    #[test]
    fn test_normal_mode_actions() {
        assert!(matches!(handle_normal_key("d", ""), Action::Cut));
        assert!(matches!(handle_normal_key("p", ""), Action::Paste));
        assert!(matches!(handle_normal_key("x", ""), Action::Delete));
        assert!(matches!(handle_normal_key(".", ""), Action::ToggleHidden));
        assert!(matches!(handle_normal_key(":", ""), Action::EnterCommandMode));
        assert!(matches!(handle_normal_key("v", ""), Action::EnterVisualMode));
        assert!(matches!(handle_normal_key("=", ""), Action::OpenFile));
    }

    #[test]
    fn test_visual_mode() {
        assert!(matches!(handle_visual_key("j"), Action::MoveCursor(1)));
        assert!(matches!(handle_visual_key("k"), Action::MoveCursor(-1)));
        assert!(matches!(handle_visual_key("y"), Action::Yank));
        assert!(matches!(handle_visual_key("d"), Action::Cut));
        assert!(matches!(handle_visual_key("\u{1b}"), Action::ExitVisualMode));
        assert!(matches!(handle_visual_key("v"), Action::ExitVisualMode));
    }

    #[test]
    fn test_command_mode() {
        assert!(matches!(handle_command_key("\u{1b}"), Action::CommandCancel));
        assert!(matches!(handle_command_key("\n"), Action::CommandExecute));
        assert!(matches!(handle_command_key("\u{8}"), Action::CommandBackspace));
    }

    #[test]
    fn test_command_mode_append() {
        match handle_command_key("a") {
            Action::CommandAppend(c) => assert_eq!(c, 'a'),
            _ => panic!("expected CommandAppend"),
        }
        match handle_command_key("Z") {
            Action::CommandAppend(c) => assert_eq!(c, 'Z'),
            _ => panic!("expected CommandAppend"),
        }
        match handle_command_key("5") {
            Action::CommandAppend(c) => assert_eq!(c, '5'),
            _ => panic!("expected CommandAppend"),
        }
    }

    #[test]
    fn test_handle_key_dispatches_correctly() {
        // Vim mode
        assert!(matches!(handle_key(Mode::Normal, "j", "", true), Action::MoveCursor(1)));
        assert!(matches!(handle_key(Mode::Visual, "y", "", true), Action::Yank));
        assert!(matches!(handle_key(Mode::Command, "\n", "", true), Action::CommandExecute));
        assert!(matches!(handle_key(Mode::Search, "\n", "", true), Action::SearchExecute));

        // Standard mode
        assert!(matches!(handle_key(Mode::Normal, "Down", "", false), Action::MoveCursor(1)));
        assert!(matches!(handle_key(Mode::Normal, "Up", "", false), Action::MoveCursor(-1)));
        assert!(matches!(handle_key(Mode::Normal, "C-c", "", false), Action::Yank));
        assert!(matches!(handle_key(Mode::Normal, "C-v", "", false), Action::Paste));
    }

    #[test]
    fn test_search_mode() {
        assert!(matches!(handle_search_key("\u{1b}"), Action::SearchCancel));
        assert!(matches!(handle_search_key("\n"), Action::SearchExecute));
        assert!(matches!(handle_search_key("\u{8}"), Action::SearchBackspace));
        match handle_search_key("a") {
            Action::SearchAppend(c) => assert_eq!(c, 'a'),
            _ => panic!("expected SearchAppend"),
        }
    }

    #[test]
    fn test_normal_mode_search() {
        assert!(matches!(handle_normal_key("/", ""), Action::EnterSearchMode));
        assert!(matches!(handle_normal_key("n", ""), Action::SearchNext));
        assert!(matches!(handle_normal_key("N", ""), Action::SearchPrev));
        assert!(matches!(handle_normal_key("C-l", ""), Action::ClearSearchHighlight));
    }

    #[test]
    fn test_normal_mode_bookmarks() {
        assert!(matches!(handle_normal_key("m", ""), Action::Pending));
        assert!(matches!(handle_normal_key("'", ""), Action::Pending));
        match handle_normal_key("a", "m") {
            Action::SetMark(c) => assert_eq!(c, 'a'),
            _ => panic!("expected SetMark"),
        }
        match handle_normal_key("z", "'") {
            Action::JumpToMark(c) => assert_eq!(c, 'z'),
            _ => panic!("expected JumpToMark"),
        }
    }

    #[test]
    fn test_normal_mode_sorting() {
        assert!(matches!(handle_normal_key("s", ""), Action::CycleSort));
        assert!(matches!(handle_normal_key("S", ""), Action::ReverseSort));
    }

    #[test]
    fn test_sort_mode() {
        assert_eq!(SortMode::Name.display(), "name");
        assert_eq!(SortMode::Size.display(), "size");
        assert_eq!(SortMode::Date.display(), "date");
        assert_eq!(SortMode::Type.display(), "type");

        assert_eq!(SortMode::Name.next(), SortMode::Size);
        assert_eq!(SortMode::Size.next(), SortMode::Date);
        assert_eq!(SortMode::Date.next(), SortMode::Type);
        assert_eq!(SortMode::Type.next(), SortMode::Name);
    }

    #[test]
    fn test_normal_mode_fold() {
        assert!(matches!(handle_normal_key("z", ""), Action::Pending));
        assert!(matches!(handle_normal_key("o", "z"), Action::FoldOpen));
        assert!(matches!(handle_normal_key("c", "z"), Action::FoldClose));
        assert!(matches!(handle_normal_key("a", "z"), Action::FoldToggle));
        assert!(matches!(handle_normal_key("O", "z"), Action::FoldOpenRecursive));
        assert!(matches!(handle_normal_key("C", "z"), Action::FoldCloseRecursive));
    }
}
