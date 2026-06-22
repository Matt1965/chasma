//! Local unit movement tick for player-issued orders (ADR-030, ADR-033 U8).

use bevy::prelude::*;

use crate::world::{
    step_all_unit_movement, DoodadCatalog, UnitCatalog, WorldData,
};

/// Advance authoritative unit movement each frame.
pub fn tick_unit_movement(
    time: Res<Time>,
    mut world: ResMut<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
) {
    step_all_unit_movement(
        &mut world,
        &unit_catalog,
        &doodad_catalog,
        time.delta_secs(),
    );
}
