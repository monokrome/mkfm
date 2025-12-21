//! Preview surface state management

use std::path::PathBuf;

use mkframe::{
    App as MkApp, AttachedAnchor, AttachedSurfaceId, QueueHandle, SubsurfaceId, WindowId,
};

use crate::app::App;
use crate::config::{OverlayConfig, OverlayPosition};
use crate::preview::PreviewCache;

/// Manages preview surface state and lifecycle
pub struct PreviewState {
    pub path: Option<PathBuf>,
    pub needs_render: bool,
    pub cache: PreviewCache,
    pub attached: Option<AttachedSurfaceId>,
    pub subsurface: Option<SubsurfaceId>,
    use_attached: bool,
}

impl PreviewState {
    pub fn new(use_attached: bool) -> Self {
        Self {
            path: None,
            needs_render: false,
            cache: PreviewCache::new(),
            attached: None,
            subsurface: None,
            use_attached,
        }
    }

    /// Update preview surface state based on current app state
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        app: &App,
        mkapp: &mut MkApp,
        qh: &QueueHandle<MkApp>,
        window_id: WindowId,
        overlay_config: &OverlayConfig,
        win_w: u32,
        win_h: u32,
    ) {
        let current_preview_file = app.current_previewable_path();
        let should_show = app.overlay_enabled && current_preview_file.is_some();

        if should_show {
            self.show_preview(
                mkapp,
                qh,
                window_id,
                overlay_config,
                &current_preview_file,
                win_w,
                win_h,
            );
        } else {
            self.hide_preview(mkapp);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_preview(
        &mut self,
        mkapp: &mut MkApp,
        qh: &QueueHandle<MkApp>,
        window_id: WindowId,
        config: &OverlayConfig,
        current_file: &Option<PathBuf>,
        win_w: u32,
        win_h: u32,
    ) {
        let have_preview = self.attached.is_some() || self.subsurface.is_some();
        let file_changed = self.path != *current_file;

        if (!have_preview || file_changed)
            && let Some(path) = current_file
        {
            let preview_width = config.max_width.resolve(win_w) as u32;
            let preview_height = config.max_height.resolve(win_h) as u32;
            let content = self.cache.get_or_load(path, preview_width, preview_height);
            let (actual_w, actual_h) = content.dimensions(preview_width, preview_height);

            if file_changed {
                self.close_surfaces(mkapp);
            }

            self.path = current_file.clone();

            if self.attached.is_none() && self.subsurface.is_none() {
                self.create_surface(
                    mkapp, qh, window_id, config, actual_w, actual_h, win_w, win_h,
                );
            }
            self.needs_render = true;
        }
    }

    fn hide_preview(&mut self, mkapp: &mut MkApp) {
        if self.attached.is_some() || self.subsurface.is_some() {
            self.close_surfaces(mkapp);
            self.path = None;
            self.needs_render = false;
            self.cache.invalidate();
        }
    }

    fn close_surfaces(&mut self, mkapp: &mut MkApp) {
        if let Some(id) = self.attached.take() {
            mkapp.close_attached_surface(id);
        }
        if let Some(id) = self.subsurface.take() {
            mkapp.close_subsurface(id);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_surface(
        &mut self,
        mkapp: &mut MkApp,
        qh: &QueueHandle<MkApp>,
        window_id: WindowId,
        config: &OverlayConfig,
        width: u32,
        height: u32,
        win_w: u32,
        win_h: u32,
    ) {
        let anchor = position_to_anchor(config.position);
        let margin = config.margin.resolve(win_w.min(win_h));
        let offset = config.offset.resolve(if is_horizontal(config.position) {
            win_h
        } else {
            win_w
        });

        if self.use_attached {
            self.attached = mkapp.create_attached_surface(qh, window_id, 0, 0, width, height);
            if let Some(id) = self.attached {
                mkapp.set_attached_surface_anchor(id, anchor, margin, offset);
            }
        } else {
            let (x, y) = calculate_subsurface_position(
                config.position,
                width,
                height,
                win_w,
                win_h,
                margin,
                offset,
            );
            self.subsurface = mkapp.create_subsurface(qh, window_id, x, y, width, height);
        }
    }
}

fn position_to_anchor(position: OverlayPosition) -> AttachedAnchor {
    match position {
        OverlayPosition::Right => AttachedAnchor::Right,
        OverlayPosition::Left => AttachedAnchor::Left,
        OverlayPosition::Top => AttachedAnchor::Top,
        OverlayPosition::Bottom => AttachedAnchor::Bottom,
    }
}

fn is_horizontal(position: OverlayPosition) -> bool {
    matches!(position, OverlayPosition::Left | OverlayPosition::Right)
}

fn calculate_subsurface_position(
    position: OverlayPosition,
    width: u32,
    height: u32,
    win_w: u32,
    win_h: u32,
    margin: i32,
    offset: i32,
) -> (i32, i32) {
    match position {
        OverlayPosition::Right => (win_w as i32 + margin, offset),
        OverlayPosition::Left => (-(width as i32) - margin, offset),
        OverlayPosition::Top => (offset, -(height as i32) - margin),
        OverlayPosition::Bottom => (offset, win_h as i32 + margin),
    }
}
