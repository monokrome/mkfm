//! Mouse event handling (click, scroll, drag)

use mkui::event::MouseEvent;
use mkui::layout::Rect;

use super::pointer_helpers::{DoubleClickChecker, PointerLayout};
use super::{App, FocusArea};
use crate::input::Action;

impl App {
    /// Handle mouse events
    pub fn handle_mouse_event(
        &mut self,
        event: &MouseEvent,
        window_width: u16,
        window_height: u16,
        ctrl_held: bool,
    ) -> bool {
        let layout = PointerLayout::calculate(
            window_height as u32,
            &self.task_list,
            &self.error_list,
        );

        match event {
            MouseEvent::Button {
                button: mkui::event::MouseButton::Left,
                state: mkui::event::KeyState::Pressed,
                col,
                row,
                ..
            } => self.handle_left_click(*col, *row, &layout, window_width, ctrl_held),
            MouseEvent::Button {
                button: mkui::event::MouseButton::Left,
                state: mkui::event::KeyState::Released,
                ..
            } => {
                self.drag_start_pos = None;
                self.dragging = false;
                false
            }
            MouseEvent::Moved { col, row } => self.handle_motion(*col as f64, *row as f64),
            MouseEvent::Scroll { delta_y, .. } => self.handle_scroll(*delta_y as f64),
            _ => false,
        }
    }

    fn handle_left_click(
        &mut self,
        col: u16,
        row: u16,
        layout: &PointerLayout,
        window_width: u16,
        ctrl_held: bool,
    ) -> bool {
        self.drag_start_pos = Some((col as f64, row as f64));
        self.dragging = false;

        let main_rows = layout.main_content_height as u16;
        let list_rows = layout.list_pane_height as u16;

        if row < main_rows {
            self.handle_browser_click(col, row, main_rows, window_width, ctrl_held)
        } else if row < main_rows + list_rows {
            self.drag_start_pos = None;
            self.focus_area = FocusArea::TaskList;
            true
        } else {
            false
        }
    }

    fn handle_browser_click(
        &mut self,
        col: u16,
        row: u16,
        main_rows: u16,
        window_width: u16,
        ctrl_held: bool,
    ) -> bool {
        let bounds = Rect::new(0, 0, window_width, main_rows);
        let click_info = self.compute_click_target(bounds, col, row);

        if let Some(entry_index) = click_info {
            self.handle_entry_click(col as f64, row as f64, entry_index, ctrl_held)
        } else {
            false
        }
    }

    fn compute_click_target(&mut self, bounds: Rect, col: u16, row: u16) -> Option<usize> {
        let header_height = 1u16;

        let (leaf_id, pane_rect) = self.splits.find_at_position(bounds, col, row)?;
        self.splits.set_focused(leaf_id);
        self.focus_area = FocusArea::Splits;

        let list_top = pane_rect.y + header_height;
        if row < list_top {
            return None;
        }

        let visual_index = (row - list_top) as usize;
        let list_height = pane_rect.height.saturating_sub(header_height) as usize;

        let browser = self.splits.get_mut(leaf_id)?;

        let scroll_offset = if browser.cursor >= list_height && list_height > 0 {
            browser.cursor - list_height + 1
        } else {
            0
        };

        let entry_index = scroll_offset + visual_index;
        if entry_index < browser.entries.len() {
            Some(entry_index)
        } else {
            None
        }
    }

    fn handle_entry_click(
        &mut self,
        x: f64,
        y: f64,
        entry_index: usize,
        ctrl_held: bool,
    ) -> bool {
        let is_double_click = self.check_double_click(x, y);

        if is_double_click {
            self.drag_start_pos = None;
            if let Some(browser) = self.browser_mut() {
                browser.cursor = entry_index;
            }
            self.execute(Action::EnterDirectory)
        } else if ctrl_held {
            self.handle_ctrl_click(entry_index)
        } else {
            if let Some(browser) = self.browser_mut() {
                browser.cursor = entry_index;
            }
            true
        }
    }

    fn handle_ctrl_click(&mut self, _entry_index: usize) -> bool {
        // Selection toggle — will be implemented with visual mode
        false
    }

    fn check_double_click(&mut self, x: f64, y: f64) -> bool {
        let checker = DoubleClickChecker::default();
        let is_double = checker.is_double_click(self.last_click_time, self.last_click_pos, x, y);
        self.last_click_time = std::time::Instant::now();
        self.last_click_pos = (x, y);
        is_double
    }

    fn handle_motion(&mut self, x: f64, y: f64) -> bool {
        if let Some(start) = self.drag_start_pos {
            let dx = (x - start.0).abs();
            let dy = (y - start.1).abs();
            if dx > 3.0 || dy > 3.0 {
                self.dragging = true;
            }
        }
        false
    }

    fn handle_scroll(&mut self, delta: f64) -> bool {
        if delta < 0.0 {
            self.execute(Action::MoveCursor(-3))
        } else if delta > 0.0 {
            self.execute(Action::MoveCursor(3))
        } else {
            false
        }
    }
}
