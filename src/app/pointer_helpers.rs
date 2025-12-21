//! Helper types and functions for pointer event handling

use crate::jobs::{ErrorListPane, TaskListPane};

pub struct PointerLayout {
    pub main_content_height: u32,
    pub list_pane_height: u32,
}

impl PointerLayout {
    pub fn calculate(
        window_height: u32,
        task_list: &TaskListPane,
        error_list: &ErrorListPane,
    ) -> Self {
        let status_height = 28u32;
        let list_pane_visible = task_list.visible || error_list.visible;
        let list_pane_height = if list_pane_visible {
            (window_height as f32 * 0.20).round() as u32
        } else {
            0
        };
        let main_content_height = window_height.saturating_sub(status_height + list_pane_height);

        Self {
            main_content_height,
            list_pane_height,
        }
    }
}

pub struct DoubleClickChecker {
    pub threshold: std::time::Duration,
    pub distance_threshold: f64,
}

impl Default for DoubleClickChecker {
    fn default() -> Self {
        Self {
            threshold: std::time::Duration::from_millis(400),
            distance_threshold: 5.0,
        }
    }
}

impl DoubleClickChecker {
    pub fn is_double_click(
        &self,
        last_time: std::time::Instant,
        last_pos: (f64, f64),
        x: f64,
        y: f64,
    ) -> bool {
        let now = std::time::Instant::now();
        let time_ok = now.duration_since(last_time) < self.threshold;
        let dx = (x - last_pos.0).abs();
        let dy = (y - last_pos.1).abs();
        let pos_ok = dx < self.distance_threshold && dy < self.distance_threshold;
        time_ok && pos_ok
    }
}
