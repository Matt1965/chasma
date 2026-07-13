use bevy::prelude::*;

/// Stable identifier for a building instance (ADR-079 B2).
///
/// Assigned monotonically by [`crate::world::WorldData`]; not derived from position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct BuildingId(pub u64);

impl BuildingId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}
