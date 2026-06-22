use bevy::prelude::*;

use crate::world::UnitId;

/// Links a derived render entity to authoritative unit data (ADR-028).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitRenderEntity {
    pub unit_id: UnitId,
}

/// Marker on the glTF scene root spawned for a unit (ADR-028).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitSceneRoot;

/// Green selection ring rendered at a unit's feet (ADR-033 U8).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitSelectionIndicator;
