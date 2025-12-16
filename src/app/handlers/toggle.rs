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

    pub fn execute_toggle_overlay(&mut self) -> bool {
        self.overlay_enabled = !self.overlay_enabled;
        true
    }

    pub fn execute_enable_overlay(&mut self) -> bool {
        self.overlay_enabled = true;
        true
    }

    pub fn execute_disable_overlay(&mut self) -> bool {
        self.overlay_enabled = false;
        true
    }
}
