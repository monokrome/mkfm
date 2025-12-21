//! Pointer event handling (click, scroll, drag)

use mkframe::{PointerButton, PointerEvent, PointerEventKind, Rect};

use super::pointer_helpers::{DoubleClickChecker, PointerLayout};
use super::{App, FocusArea};
use crate::input::Action;

impl App {
    /// Handle pointer events (click, scroll, drag)
    pub fn handle_pointer_event(
        &mut self,
        event: &PointerEvent,
        window_width: u32,
        window_height: u32,
        ctrl_held: bool,
    ) -> bool {
        let layout = PointerLayout::calculate(window_height, &self.task_list, &self.error_list);

        match &event.kind {
            PointerEventKind::Press(PointerButton::Left) => {
                self.handle_left_click(event.x, event.y, &layout, window_width, ctrl_held)
            }
            PointerEventKind::Release(PointerButton::Left) => {
                self.drag_start_pos = None;
                self.dragging = false;
                false
            }
            PointerEventKind::Motion => self.handle_motion(event.x, event.y),
            PointerEventKind::Scroll { dy, .. } => self.handle_scroll(*dy as f64),
            _ => false,
        }
    }

    fn handle_left_click(
        &mut self,
        x: f64,
        y: f64,
        layout: &PointerLayout,
        window_width: u32,
        ctrl_held: bool,
    ) -> bool {
        self.drag_start_pos = Some((x, y));
        self.dragging = false;

        if y < layout.main_content_height as f64 {
            self.handle_browser_click(x, y, layout.main_content_height, window_width, ctrl_held)
        } else if y < (layout.main_content_height + layout.list_pane_height) as f64 {
            self.drag_start_pos = None;
            self.focus_area = FocusArea::TaskList;
            true
        } else {
            false
        }
    }

    fn handle_browser_click(
        &mut self,
        x: f64,
        y: f64,
        main_content_height: u32,
        window_width: u32,
        ctrl_held: bool,
    ) -> bool {
        let bounds = Rect::new(0, 0, window_width, main_content_height);
        let click_info = self.compute_click_target(bounds, x, y);

        if let Some((entry_index, _)) = click_info {
            self.handle_entry_click(x, y, entry_index, ctrl_held)
        } else {
            false
        }
    }

    fn compute_click_target(&mut self, bounds: Rect, x: f64, y: f64) -> Option<(usize, bool)> {
        let line_height = 24i32;
        let header_height = 32i32;

        let (leaf_id, pane_rect) = self.splits.find_at_position(bounds, x, y)?;
        self.splits.set_focused(leaf_id);
        self.focus_area = FocusArea::Splits;

        let inner_y = pane_rect.y + 1;
        let list_top = inner_y + header_height;
        let list_height = (pane_rect.height as i32 - 2 - header_height).max(0);
        let visible_lines = (list_height / line_height).max(0) as usize;

        let relative_y = y as i32 - list_top;
        if relative_y < 0 {
            return None;
        }

        let visual_index = (relative_y / line_height) as usize;
        let browser = self.splits.get_mut(leaf_id)?;

        let scroll_offset = if browser.cursor >= visible_lines && visible_lines > 0 {
            browser.cursor - visible_lines + 1
        } else {
            0
        };

        let entry_index = scroll_offset + visual_index;
        if entry_index < browser.entries.len() {
            Some((entry_index, false))
        } else {
            None
        }
    }

    fn handle_entry_click(&mut self, x: f64, y: f64, entry_index: usize, ctrl_held: bool) -> bool {
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
            self.handle_single_click(entry_index)
        }
    }

    fn check_double_click(&mut self, x: f64, y: f64) -> bool {
        let checker = DoubleClickChecker::default();
        let is_double = checker.is_double_click(self.last_click_time, self.last_click_pos, x, y);
        self.last_click_time = std::time::Instant::now();
        self.last_click_pos = (x, y);
        is_double
    }

    fn handle_ctrl_click(&mut self, entry_index: usize) -> bool {
        if let Some(browser) = self.browser_mut() {
            browser.cursor = entry_index;
        }
        self.selection.toggle(entry_index);
        true
    }

    fn handle_single_click(&mut self, entry_index: usize) -> bool {
        self.selection.clear();
        if let Some(browser) = self.browser_mut() {
            browser.cursor = entry_index;
        }
        true
    }

    pub(super) fn handle_motion(&mut self, x: f64, y: f64) -> bool {
        if let Some((start_x, start_y)) = self.drag_start_pos
            && !self.dragging
        {
            let drag_threshold = 8.0;
            let dx = (x - start_x).abs();
            let dy = (y - start_y).abs();
            let distance = (dx * dx + dy * dy).sqrt();

            if distance > drag_threshold {
                self.dragging = true;
            }
        }
        false
    }

    pub(super) fn handle_scroll(&mut self, dy: f64) -> bool {
        if let Some(browser) = self.browser_mut() {
            if dy < 0.0 {
                browser.move_cursor(-3);
            } else if dy > 0.0 {
                browser.move_cursor(3);
            }
            return true;
        }
        false
    }
}
