use wayland_client::{Connection, QueueHandle, protocol::wl_surface::WlSurface};

// Generate the protocol code using wayland-scanner macros
// Path is relative to crate root
pub mod protocol {
    #![allow(dead_code, non_camel_case_types, unused_imports, clippy::all)]

    use wayland_client;
    use wayland_client::protocol::*;
    // Import xdg_toplevel for the parent argument type
    pub use wayland_protocols::xdg::shell::client::xdg_toplevel;

    pub mod __interfaces {
        use wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        // Import xdg_toplevel interface from wayland-protocols
        pub use wayland_protocols::xdg::shell::client::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocol/wlr-attached-surface-unstable-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocol/wlr-attached-surface-unstable-v1.xml");
}

use protocol::zwlr_attached_surface_manager_v1::ZwlrAttachedSurfaceManagerV1;
use protocol::zwlr_attached_surface_v1::ZwlrAttachedSurfaceV1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttachedSurfaceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    None,
    Top,
    Bottom,
    Left,
    Right,
}

impl Anchor {
    fn to_protocol(self) -> u32 {
        match self {
            Anchor::None => 0,
            Anchor::Top => 1,
            Anchor::Bottom => 2,
            Anchor::Left => 3,
            Anchor::Right => 4,
        }
    }
}

pub struct AttachedSurface {
    pub id: AttachedSurfaceId,
    pub surface: WlSurface,
    pub attached: ZwlrAttachedSurfaceV1,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub dirty: bool,
    pub configured: bool,
    pub pending_configure: Option<(u32, u32, u32)>, // serial, width, height
}

impl AttachedSurface {
    pub fn wl_surface(&self) -> &WlSurface {
        &self.surface
    }

    pub fn set_anchor(&self, anchor: Anchor, margin: i32, offset: i32) {
        use protocol::zwlr_attached_surface_v1::Anchor as ProtoAnchor;
        let proto_anchor = match anchor {
            Anchor::None => ProtoAnchor::None,
            Anchor::Top => ProtoAnchor::Top,
            Anchor::Bottom => ProtoAnchor::Bottom,
            Anchor::Left => ProtoAnchor::Left,
            Anchor::Right => ProtoAnchor::Right,
        };
        self.attached.set_anchor(proto_anchor, margin, offset);
    }

    pub fn set_position(&self, x: i32, y: i32) {
        self.attached.set_position(x, y);
    }

    pub fn set_size(&self, width: u32, height: u32) {
        self.attached.set_size(width, height);
    }

    pub fn ack_configure(&mut self, serial: u32) {
        self.attached.ack_configure(serial);
        self.configured = true;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

#[derive(Clone)]
pub struct AttachedSurfaceManager {
    manager: ZwlrAttachedSurfaceManagerV1,
}

impl AttachedSurfaceManager {
    pub fn new(manager: ZwlrAttachedSurfaceManagerV1) -> Self {
        Self { manager }
    }

    pub fn inner(&self) -> &ZwlrAttachedSurfaceManagerV1 {
        &self.manager
    }
}

// Data attached to attached surface protocol objects
pub struct AttachedSurfaceData {
    pub id: AttachedSurfaceId,
}

pub trait AttachedSurfaceHandler {
    fn configure(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface_id: AttachedSurfaceId,
        serial: u32,
        width: u32,
        height: u32,
    ) where
        Self: Sized;

    fn closed(&mut self, conn: &Connection, qh: &QueueHandle<Self>, surface_id: AttachedSurfaceId)
    where
        Self: Sized;
}
