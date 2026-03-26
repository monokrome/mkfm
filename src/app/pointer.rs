//! Pointer event handling (click, scroll, drag)
//!
//! Stubbed during mkui migration. Will be reimplemented using mkui's
//! MouseEvent types.

use super::App;

impl App {
    /// Handle mouse events (stubbed during migration)
    pub fn handle_mouse_event(
        &mut self,
        _event: &mkui::event::MouseEvent,
        _window_width: u16,
        _window_height: u16,
        _ctrl_held: bool,
    ) -> bool {
        false
    }
}
