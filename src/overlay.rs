//! Wayland overlay management for preview surfaces
//!
//! Creates and manages attached surfaces for preview rendering on
//! Wayland compositors that support the wlr-attached-surface protocol.

use std::ptr::NonNull;

use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy,
    globals::registry_queue_init,
    protocol::{wl_compositor::WlCompositor, wl_registry, wl_surface::WlSurface},
};
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;

use crate::attached_surface::{
    protocol::zwlr_attached_surface_manager_v1::ZwlrAttachedSurfaceManagerV1,
    protocol::zwlr_attached_surface_v1::ZwlrAttachedSurfaceV1,
    Anchor, AttachedSurface, AttachedSurfaceData, AttachedSurfaceId, AttachedSurfaceManager,
};

/// Manages the Wayland overlay for preview rendering
pub struct OverlayManager {
    connection: Connection,
    event_queue: EventQueue<OverlayState>,
    state: OverlayState,
}

struct OverlayState {
    compositor: Option<WlCompositor>,
    manager: Option<AttachedSurfaceManager>,
    surface: Option<AttachedSurface>,
    next_id: u64,
}

impl OverlayManager {
    /// Try to create an overlay manager from a winit window.
    /// Returns None if not on Wayland or the extension isn't available.
    pub fn new(window: &winit::window::Window) -> Option<Self> {
        use rwh_06::HasDisplayHandle;

        let display_handle = window.display_handle().ok()?;
        let raw = display_handle.as_raw();

        let wl_display = match raw {
            rwh_06::RawDisplayHandle::Wayland(handle) => handle.display,
            _ => return None,
        };

        // Safety: winit guarantees the display pointer is valid for the window's lifetime
        let backend = unsafe {
            wayland_backend::client::Backend::from_foreign_display(wl_display.as_ptr().cast())
        };
        let connection = Connection::from_backend(backend);

        let (globals, event_queue) = registry_queue_init::<OverlayState>(&connection).ok()?;

        let compositor: WlCompositor = globals
            .bind(&event_queue.handle(), 4..=6, ())
            .ok()?;

        let manager: Option<ZwlrAttachedSurfaceManagerV1> = globals
            .bind(&event_queue.handle(), 1..=1, ())
            .ok();

        let attached_manager = manager.map(AttachedSurfaceManager::new);

        Some(OverlayManager {
            connection,
            event_queue,
            state: OverlayState {
                compositor: Some(compositor),
                manager: attached_manager,
                surface: None,
                next_id: 0,
            },
        })
    }

    /// Check if the compositor supports attached surfaces
    pub fn has_attached_surface(&self) -> bool {
        self.state.manager.is_some()
    }

    /// Create an attached surface for the given toplevel.
    /// `toplevel_ptr` comes from `WindowExtWayland::xdg_toplevel()`.
    pub fn create_surface(
        &mut self,
        toplevel_ptr: NonNull<std::ffi::c_void>,
        anchor: Anchor,
        margin: i32,
        offset: i32,
        width: u32,
        height: u32,
    ) -> Option<AttachedSurfaceId> {
        let manager = self.state.manager.as_ref()?;
        let compositor = self.state.compositor.as_ref()?;
        let qh = self.event_queue.handle();

        // Create a new wl_surface for the overlay
        let surface = compositor.create_surface(&qh, ());

        // Reconstruct the xdg_toplevel from winit's raw pointer.
        // Both connections share the same wl_display, so object IDs are valid.
        let toplevel_id = unsafe {
            wayland_backend::client::ObjectId::from_ptr(
                XdgToplevel::interface(),
                toplevel_ptr.as_ptr().cast(),
            )
        }.ok()?;
        let toplevel = XdgToplevel::from_id(&self.connection, toplevel_id).ok()?;

        let id = AttachedSurfaceId(self.state.next_id);
        self.state.next_id += 1;

        let proto_anchor = match anchor {
            Anchor::None => crate::attached_surface::protocol::zwlr_attached_surface_v1::Anchor::None,
            Anchor::Top => crate::attached_surface::protocol::zwlr_attached_surface_v1::Anchor::Top,
            Anchor::Bottom => crate::attached_surface::protocol::zwlr_attached_surface_v1::Anchor::Bottom,
            Anchor::Left => crate::attached_surface::protocol::zwlr_attached_surface_v1::Anchor::Left,
            Anchor::Right => crate::attached_surface::protocol::zwlr_attached_surface_v1::Anchor::Right,
        };

        let attached = manager.inner().get_attached_surface(
            &surface,
            &toplevel,
            &qh,
            AttachedSurfaceData { id },
        );

        attached.set_anchor(proto_anchor, margin, offset);
        attached.set_size(width, height);
        surface.commit();

        self.state.surface = Some(AttachedSurface {
            id,
            surface,
            attached,
            x: 0,
            y: 0,
            width,
            height,
            dirty: true,
            configured: false,
            pending_configure: None,
        });

        Some(id)
    }

    /// Destroy the current overlay surface
    pub fn destroy_surface(&mut self) {
        if let Some(surface) = self.state.surface.take() {
            surface.attached.destroy();
            surface.surface.destroy();
        }
    }

    /// Get the current overlay surface if it exists and is configured
    pub fn surface(&self) -> Option<&AttachedSurface> {
        self.state.surface.as_ref().filter(|s| s.configured)
    }

    /// Get mutable access to the overlay surface
    pub fn surface_mut(&mut self) -> Option<&mut AttachedSurface> {
        self.state.surface.as_mut().filter(|s| s.configured)
    }

    /// Dispatch pending Wayland events
    pub fn dispatch(&mut self) {
        let _ = self.event_queue.dispatch_pending(&mut self.state);
    }
}

// Wayland dispatch implementations

impl Dispatch<wl_registry::WlRegistry, wayland_client::globals::GlobalListContents> for OverlayState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &wayland_client::globals::GlobalListContents,
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlCompositor, ()> for OverlayState {
    fn event(
        _state: &mut Self,
        _proxy: &WlCompositor,
        _event: <WlCompositor as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSurface, ()> for OverlayState {
    fn event(
        _state: &mut Self,
        _proxy: &WlSurface,
        _event: <WlSurface as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrAttachedSurfaceManagerV1, ()> for OverlayState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrAttachedSurfaceManagerV1,
        _event: <ZwlrAttachedSurfaceManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrAttachedSurfaceV1, AttachedSurfaceData> for OverlayState {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrAttachedSurfaceV1,
        event: <ZwlrAttachedSurfaceV1 as Proxy>::Event,
        data: &AttachedSurfaceData,
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        use crate::attached_surface::protocol::zwlr_attached_surface_v1::Event;
        match event {
            Event::Configure { serial, width, height } => {
                if let Some(ref mut surface) = state.surface {
                    if surface.id == data.id {
                        surface.ack_configure(serial);
                        surface.width = width;
                        surface.height = height;
                        surface.dirty = true;
                    }
                }
            }
            Event::Closed => {
                if let Some(ref surface) = state.surface {
                    if surface.id == data.id {
                        state.surface = None;
                    }
                }
            }
            _ => {}
        }
    }
}

// XdgToplevel dispatch — needed since we reconstruct it from a foreign pointer
impl Dispatch<XdgToplevel, ()> for OverlayState {
    fn event(
        _state: &mut Self,
        _proxy: &XdgToplevel,
        _event: <XdgToplevel as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        // Events handled by winit, not us
    }
}
