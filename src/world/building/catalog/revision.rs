use bevy::prelude::*;

/// Monotonic revision bumped when building catalog content changes (NV1.6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect, Resource)]
pub struct BuildingCatalogRevision(pub u64);
