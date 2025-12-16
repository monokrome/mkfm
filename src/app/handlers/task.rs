//! Task and error list action handlers

use crate::app::{App, FocusArea};

impl App {
    pub fn execute_next_task(&mut self) -> bool {
        if !self.task_list.visible {
            self.task_list.show();
        } else {
            let job_count = self.job_queue.all_jobs().len();
            if job_count > 0 && self.task_list.cursor < job_count - 1 {
                self.task_list.cursor += 1;
            }
        }
        true
    }

    pub fn execute_prev_task(&mut self) -> bool {
        if !self.task_list.visible {
            self.task_list.show();
        } else if self.task_list.cursor > 0 {
            self.task_list.cursor -= 1;
        }
        true
    }

    pub fn execute_toggle_task_list(&mut self) -> bool {
        self.task_list.toggle();
        if !self.task_list.visible && !self.error_list.visible {
            self.focus_area = FocusArea::Splits;
        }
        true
    }

    pub fn execute_next_error(&mut self) -> bool {
        if !self.error_list.visible {
            self.error_list.show();
        } else {
            let error_count = self.job_queue.failed_count();
            if error_count > 0 && self.error_list.cursor < error_count - 1 {
                self.error_list.cursor += 1;
            }
        }
        true
    }

    pub fn execute_prev_error(&mut self) -> bool {
        if !self.error_list.visible {
            self.error_list.show();
        } else if self.error_list.cursor > 0 {
            self.error_list.cursor -= 1;
        }
        true
    }

    pub fn execute_toggle_error_list(&mut self) -> bool {
        self.error_list.toggle();
        if !self.task_list.visible && !self.error_list.visible {
            self.focus_area = FocusArea::Splits;
        }
        true
    }

    pub fn execute_toggle_feature_list(&mut self) -> bool {
        self.feature_pane.toggle();
        if self.feature_pane.visible {
            self.focus_area = FocusArea::FeatureList;
        } else {
            self.focus_area = FocusArea::Splits;
        }
        true
    }
}
