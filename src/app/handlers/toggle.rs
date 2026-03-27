//! Toggle action handlers

use crate::app::App;

impl App {
    pub fn execute_toggle_hidden(&mut self) -> bool {
        if let Some(browser) = self.browser_mut() {
            browser.toggle_hidden();
        }
        true
    }

    pub fn execute_enable_hidden(&mut self) -> bool {
        if let Some(browser) = self.browser_mut()
            && !browser.show_hidden
        {
            browser.toggle_hidden();
        }
        true
    }

    pub fn execute_disable_hidden(&mut self) -> bool {
        if let Some(browser) = self.browser_mut()
            && browser.show_hidden
        {
            browser.toggle_hidden();
        }
        true
    }

    pub fn execute_toggle_preview(&mut self) -> bool {
        self.preview_enabled = !self.preview_enabled;
        true
    }

    pub fn execute_enable_preview(&mut self) -> bool {
        self.preview_enabled = true;
        true
    }

    pub fn execute_disable_preview(&mut self) -> bool {
        self.preview_enabled = false;
        true
    }
}
