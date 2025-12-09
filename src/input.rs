#[derive(Clone, Copy, PartialEq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Visual,
    Command,
}

impl Mode {
    pub fn display(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Visual => "VISUAL",
            Mode::Command => "COMMAND",
        }
    }
}

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
    ToggleHidden,
    EnableHidden,
    DisableHidden,
    ToggleOverlay,
    EnableOverlay,
    DisableOverlay,
}

pub fn handle_normal_key(key: &str, pending: &str) -> Action {
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

        // "yy" yanks
        ("y", "y") => Action::Yank,
        ("", "y") => Action::Pending,

        // Start bracket sequences
        ("", "[") => Action::Pending,
        ("", "]") => Action::Pending,

        // Single key commands
        (_, "j") => Action::MoveCursor(1),
        (_, "k") => Action::MoveCursor(-1),
        (_, "g") => Action::CursorToTop,
        (_, "G") => Action::CursorToBottom,
        (_, "l") | (_, "\n") => Action::EnterDirectory,
        (_, "h") | (_, "-") => Action::ParentDirectory,
        (_, "v") => Action::EnterVisualMode,
        (_, "d") => Action::Cut,
        (_, "p") => Action::Paste,
        (_, "x") => Action::Delete,
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

pub fn handle_key(mode: Mode, key: &str, pending: &str) -> Action {
    match mode {
        Mode::Normal => handle_normal_key(key, pending),
        Mode::Visual => handle_visual_key(key),
        Mode::Command => handle_command_key(key),
    }
}
