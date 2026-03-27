//! Preview surface state management
//!
//! Manages preview rendering path selection:
//! - Inline: renders inside the browser pane (TUI, or GUI without overlay)
//! - Overlay: renders in a Wayland attached surface (GUI with extension support)

use std::path::PathBuf;

use crate::attached_surface::Anchor;
use crate::overlay::OverlayManager;
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
    pub cache: PreviewCache,
    pub mode: PreviewMode,
    pub overlay: Option<OverlayManager>,
    pub current_path: Option<PathBuf>,
    pub needs_render: bool,
}

impl PreviewState {
    pub fn new() -> Self {
        PreviewState {
            cache: PreviewCache::new(),
            mode: PreviewMode::Inline,
            overlay: None,
            current_path: None,
            needs_render: false,
        }
    }

    /// Detect overlay support and initialize if available.
    /// Call once after the renderer is created.
    pub fn init_overlay(&mut self, renderer: &dyn mkui::render::Renderer) {
        use mkui::gui::WgpuRenderer;

        let Some(gpu) = renderer.as_any().downcast_ref::<WgpuRenderer>() else {
            return;
        };

        let overlay = OverlayManager::new(gpu.window());
        if let Some(ref overlay) = overlay {
            if overlay.has_attached_surface() {
                self.mode = PreviewMode::Overlay;
            }
        }
        self.overlay = overlay;
    }

    /// Create or update the overlay surface for the current file.
    /// Call from the render loop when the cursor changes.
    pub fn update_overlay(
        &mut self,
        renderer: &dyn mkui::render::Renderer,
        file_path: Option<&std::path::Path>,
        preview_enabled: bool,
    ) {
        // Track if the file changed
        let path_changed = match (&self.current_path, file_path) {
            (Some(old), Some(new)) => old != new,
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (None, None) => false,
        };

        self.current_path = file_path.map(|p| p.to_path_buf());

        if !preview_enabled || self.mode != PreviewMode::Overlay {
            // Destroy overlay if it exists but shouldn't
            if let Some(ref mut overlay) = self.overlay {
                overlay.destroy_surface();
            }
            return;
        }

        let Some(ref mut overlay) = self.overlay else {
            return;
        };

        overlay.dispatch();

        // Only show overlay for previewable files
        let Some(path) = file_path else {
            overlay.destroy_surface();
            return;
        };

        if !PreviewCache::is_previewable(path) {
            overlay.destroy_surface();
            return;
        }

        // Get window bounds for constraining overlay size
        use mkui::gui::WgpuRenderer;
        let Some(gpu) = renderer.as_any().downcast_ref::<WgpuRenderer>() else {
            return;
        };
        let win_size = gpu.window().inner_size();
        let anchor = Anchor::Right; // TODO: make configurable
        let margin = 8i32;

        // Max available space from the anchor edge
        let (max_w, max_h) = match anchor {
            Anchor::Left | Anchor::Right => (
                win_size.width / 2,
                win_size.height.saturating_sub(margin as u32 * 2),
            ),
            Anchor::Top | Anchor::Bottom => (
                win_size.width.saturating_sub(margin as u32 * 2),
                win_size.height / 2,
            ),
            Anchor::None => (
                win_size.width / 2,
                win_size.height / 2,
            ),
        };

        // Load content to get natural dimensions, bounded by available space
        let content = self.cache.get_or_load(path, max_w.max(1), max_h.max(1));
        let (desired_w, desired_h) = content.dimensions(max_w.max(1), max_h.max(1));

        if desired_w == 0 || desired_h == 0 {
            overlay.destroy_surface();
            return;
        }

        // Create or resize surface to match content
        if overlay.surface().is_none() {
            use winit::platform::wayland::WindowExtWayland;

            if let Some(toplevel_ptr) = gpu.window().xdg_toplevel() {
                overlay.create_surface(
                    toplevel_ptr,
                    anchor,
                    margin,
                    0,
                    desired_w,
                    desired_h,
                );
            }
        } else if let Some(surface) = overlay.surface() {
            if surface.width != desired_w || surface.height != desired_h {
                overlay.resize_surface(desired_w, desired_h);
                self.needs_render = true;
            }
        }

        if path_changed {
            self.needs_render = true;
        }
    }

    /// Render preview content to the overlay surface.
    /// Returns true if the overlay handled rendering (caller should skip inline).
    pub fn render_overlay(&mut self) -> bool {
        if self.mode != PreviewMode::Overlay {
            return false;
        }

        let overlay = match self.overlay {
            Some(ref mut o) => o,
            None => return false,
        };

        // Check if surface exists and needs rendering
        let (width, height, needs_work) = match overlay.surface() {
            Some(s) => (s.width, s.height, self.needs_render || s.dirty),
            None => return false,
        };

        if !needs_work {
            return true;
        }

        let Some(ref path) = self.current_path else {
            return false;
        };

        let content = self.cache.get_or_load(path, width, height);
        let buffer = render_to_pixel_buffer(content, width, height);
        overlay.submit_buffer(&buffer, width, height);

        if let Some(surface) = overlay.surface_mut() {
            surface.dirty = false;
        }
        self.needs_render = false;
        true
    }

    /// Check if overlay is active (caller should skip inline preview)
    pub fn is_overlay_active(&self) -> bool {
        self.mode == PreviewMode::Overlay
            && self.overlay.as_ref().is_some_and(|o| o.surface().is_some())
    }
}

/// Render preview content to a BGRA pixel buffer for Wayland submission
fn render_to_pixel_buffer(
    content: &crate::preview::PreviewContent,
    width: u32,
    height: u32,
) -> Vec<u8> {
    use crate::preview::PreviewContent;

    let size = (width * height * 4) as usize;
    let mut buffer = vec![0u8; size];

    // Transparent background — overlay is content only, no chrome
    // Buffer is already zeroed (BGRA with alpha=0)

    if let PreviewContent::Image { data, width: img_w, height: img_h } = content {
        let scale_x = *img_w as f32 / width as f32;
        let scale_y = *img_h as f32 / height as f32;
        let scale = scale_x.max(scale_y).max(1.0);

        let dst_w = (*img_w as f32 / scale) as u32;
        let dst_h = (*img_h as f32 / scale) as u32;
        let off_x = width.saturating_sub(dst_w) / 2;
        let off_y = height.saturating_sub(dst_h) / 2;

        for dy in 0..dst_h.min(height) {
            for dx in 0..dst_w.min(width) {
                let sx = (dx as f32 * scale) as u32;
                let sy = (dy as f32 * scale) as u32;
                if sx < *img_w && sy < *img_h {
                    let src_idx = ((sy * img_w + sx) * 4) as usize;
                    let dst_idx = (((off_y + dy) * width + (off_x + dx)) * 4) as usize;
                    if src_idx + 3 < data.len() && dst_idx + 3 < buffer.len() {
                        buffer[dst_idx] = data[src_idx + 2];     // B
                        buffer[dst_idx + 1] = data[src_idx + 1]; // G
                        buffer[dst_idx + 2] = data[src_idx];     // R
                        buffer[dst_idx + 3] = data[src_idx + 3]; // A
                    }
                }
            }
        }
    }

    buffer
}
