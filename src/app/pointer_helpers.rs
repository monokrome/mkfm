//! Helper types for pointer event handling

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
        let status_height = 1u32;
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
