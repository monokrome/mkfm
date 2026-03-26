//! Preview surface state management
//!
//! Manages preview rendering path selection:
//! - Inline: renders inside the browser pane (TUI, or GUI without overlay)
//! - Overlay: renders in a Wayland attached surface (GUI with extension support)

use std::path::PathBuf;

use crate::preview::PreviewCache;

/// Preview rendering mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewMode {
    /// Render inline within the browser pane
    Inline,
    /// Render in a Wayland attached surface overlay
    Overlay,
}

/// Manages preview surface lifecycle
pub struct PreviewState {
    pub path: Option<PathBuf>,
    pub cache: PreviewCache,
    pub mode: PreviewMode,
    pub needs_render: bool,
}

impl PreviewState {
    pub fn new() -> Self {
        PreviewState {
            path: None,
            cache: PreviewCache::new(),
            mode: PreviewMode::Inline,
            needs_render: false,
        }
    }

    /// Check if overlay mode is available and configure accordingly.
    /// Call this once after the renderer is created.
    pub fn detect_overlay_support(&mut self, renderer: &dyn mkui::render::Renderer) {
        use mkui::gui::WgpuRenderer;

        // Check if we're in GUI mode with Wayland
        if let Some(gpu) = renderer.as_any().downcast_ref::<WgpuRenderer>() {
            use winit::platform::wayland::WindowExtWayland;
            if gpu.window().xdg_toplevel().is_some() {
                // Wayland GUI — overlay possible if compositor supports it
                // For now, stay inline until we negotiate the protocol
                self.mode = PreviewMode::Inline;
            }
        }
    }
}
