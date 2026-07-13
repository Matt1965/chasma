use bevy::prelude::*;

/// Canonical navigable region identifier (ADR-083 B6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct SpaceId(pub u32);

impl SpaceId {
    /// Global exterior / terrain surface (ADR-083).
    pub const SURFACE: SpaceId = SpaceId(0);

    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub fn is_surface(self) -> bool {
        self == Self::SURFACE
    }
}

impl Default for SpaceId {
    fn default() -> Self {
        Self::SURFACE
    }
}

/// Stable portal edge identifier (ADR-083 B6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct PortalId(pub u32);

impl PortalId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}
