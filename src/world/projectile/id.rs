use bevy::prelude::*;

/// Stable identifier for a projectile instance (ADR-060 C7).
///
/// Assigned monotonically by [`crate::world::WorldData`] for deterministic
/// simulation and trace correlation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct ProjectileId(pub u64);

impl ProjectileId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}
