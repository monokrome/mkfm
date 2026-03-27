//! Preview surface state management
//!
//! Manages preview rendering path selection:
//! - Inline: renders inside the browser pane (TUI, or GUI without overlay)
//! - Overlay: renders in a Wayland attached surface (GUI with extension support)

use std::path::PathBuf;

use crate::attached_surface::Anchor;
use crate::overlay::OverlayManager;
use crate::preview::PreviewCache;

/// Preview rendering mode — determined once at startup, never changes
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
    /// Create for TUI — always inline, no overlay detection needed
    pub fn new_inline() -> Self {
        PreviewState {
            cache: PreviewCache::new(),
            mode: PreviewMode::Inline,
            overlay: None,
            current_path: None,
            needs_render: false,
        }
    }

    /// Create for GUI — detect overlay support once, set mode permanently
    pub fn new_for_renderer(renderer: &dyn mkui::render::Renderer) -> Self {
        use mkui::gui::WgpuRenderer;

        let mut state = PreviewState {
            cache: PreviewCache::new(),
            mode: PreviewMode::Inline, // default if no overlay support
            overlay: None,
            current_path: None,
            needs_render: false,
        };

        if let Some(gpu) = renderer.as_any().downcast_ref::<WgpuRenderer>() {
            let overlay = OverlayManager::new(gpu.window());
            if let Some(ref overlay) = overlay {
                if overlay.has_attached_surface() {
                    state.mode = PreviewMode::Overlay;
                }
            }
            state.overlay = overlay;
        }

        state
    }

    /// Is the overlay surface currently showing content?
    pub fn is_overlay_active(&self) -> bool {
        self.mode == PreviewMode::Overlay
            && self.overlay.as_ref().is_some_and(|o| o.surface().is_some())
    }

    /// Update the overlay surface for the current file.
    /// Only does anything in Overlay mode.
    pub fn update_overlay(
        &mut self,
        renderer: &dyn mkui::render::Renderer,
        file_path: Option<&std::path::Path>,
        preview_enabled: bool,
    ) {
        if self.mode != PreviewMode::Overlay {
            return;
        }

        if !preview_enabled {
            if let Some(ref mut overlay) = self.overlay {
                overlay.destroy_surface();
            }
            return;
        }

        let Some(ref mut overlay) = self.overlay else {
            return;
        };

        overlay.dispatch();

        // Destroy surface when no file or not previewable
        let Some(path) = file_path else {
            overlay.destroy_surface();
            return;
        };

        if !PreviewCache::is_previewable(path) {
            overlay.destroy_surface();
            return;
        }

        // Track file changes
        let path_changed = self.current_path.as_deref() != Some(path);
        self.current_path = Some(path.to_path_buf());

        // Get window size for overlay bounds
        use mkui::gui::WgpuRenderer;
        let Some(gpu) = renderer.as_any().downcast_ref::<WgpuRenderer>() else {
            return;
        };
        let win_size = gpu.window().inner_size();
        let anchor = Anchor::Right; // TODO: make configurable
        let margin = 8i32;

        let (desired_w, desired_h) = match anchor {
            Anchor::Left | Anchor::Right => (
                win_size.width / 2,
                win_size.height.saturating_sub(margin as u32 * 2),
            ),
            Anchor::Top | Anchor::Bottom => (
                win_size.width.saturating_sub(margin as u32 * 2),
                win_size.height / 2,
            ),
            Anchor::None => (win_size.width / 2, win_size.height / 2),
        };

        let desired_w = desired_w.max(1);
        let desired_h = desired_h.max(1);

        // Create surface if needed
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
            // Resize if window dimensions changed
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
    /// Returns true if the overlay handled rendering.
    pub fn render_overlay(&mut self) -> bool {
        if self.mode != PreviewMode::Overlay {
            return false;
        }

        let overlay = match self.overlay {
            Some(ref mut o) => o,
            None => return false,
        };

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
}

fn render_to_pixel_buffer(
    content: &crate::preview::PreviewContent,
    width: u32,
    height: u32,
) -> Vec<u8> {
    use crate::preview::PreviewContent;

    let size = (width * height * 4) as usize;
    // Transparent background — overlay is content only
    let mut buffer = vec![0u8; size];

    if let PreviewContent::Image {
        data,
        width: img_w,
        height: img_h,
    } = content
    {
        let scale_x = *img_w as f32 / width as f32;
        let scale_y = *img_h as f32 / height as f32;
        let scale = scale_x.max(scale_y).max(1.0);

        let dst_w = (*img_w as f32 / scale) as u32;
        let dst_h = (*img_h as f32 / scale) as u32;
        let off_x = width.saturating_sub(dst_w) / 2;
        let off_y = height.saturating_sub(dst_h) / 2;

        for dy in 0..dst_h.min(height) {
            for dx in 0..dst_w.min(width) {
                let src_x = (dx as f32 * scale) as u32;
                let src_y = (dy as f32 * scale) as u32;

                if src_x < *img_w && src_y < *img_h {
                    let src_idx = ((src_y * img_w + src_x) * 3) as usize;
                    let dst_x = dx + off_x;
                    let dst_y = dy + off_y;

                    if dst_x < width && dst_y < height && src_idx + 2 < data.len() {
                        let dst_idx = ((dst_y * width + dst_x) * 4) as usize;
                        // BGRA for Wayland ARGB8888
                        buffer[dst_idx] = data[src_idx + 2]; // B
                        buffer[dst_idx + 1] = data[src_idx + 1]; // G
                        buffer[dst_idx + 2] = data[src_idx]; // R
                        buffer[dst_idx + 3] = 255; // A
                    }
                }
            }
        }
    }

    if let PreviewContent::Text(lines) = content {
        // Simple text rendering — each char is ~8x16 pixels
        let char_w = 8u32;
        let char_h = 16u32;

        for (line_idx, line) in lines.iter().enumerate() {
            let y_start = line_idx as u32 * char_h;
            if y_start >= height {
                break;
            }

            for (char_idx, _ch) in line.chars().enumerate() {
                let x_start = char_idx as u32 * char_w;
                if x_start >= width {
                    break;
                }

                // Draw a simple white block for each character
                for py in 0..char_h.min(height - y_start) {
                    for px in 0..char_w.min(width - x_start) {
                        let dst_idx = (((y_start + py) * width + (x_start + px)) * 4) as usize;
                        if dst_idx + 3 < buffer.len() {
                            buffer[dst_idx] = 200; // B
                            buffer[dst_idx + 1] = 200; // G
                            buffer[dst_idx + 2] = 200; // R
                            buffer[dst_idx + 3] = 255; // A
                        }
                    }
                }
            }
        }
    }

    buffer
}
