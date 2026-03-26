//! Preview surface state management
//!
//! Stubbed during mkui migration. Preview rendering will use renderer
//! downcasting for attached surface support on GUI backends.

use std::path::PathBuf;

use crate::preview::PreviewCache;

/// Manages preview surface lifecycle
pub struct PreviewState {
    pub path: Option<PathBuf>,
    pub cache: PreviewCache,
    pub needs_render: bool,
}

impl PreviewState {
    pub fn new(_has_attached_surface: bool) -> Self {
        PreviewState {
            path: None,
            cache: PreviewCache::new(),
            needs_render: false,
        }
    }

    pub fn update(
        &mut self,
        _app: &crate::app::App,
        _overlay_config: &crate::config::OverlayConfig,
        _win_w: u16,
        _win_h: u16,
    ) {
        // Preview rendering stubbed during migration
    }
}
