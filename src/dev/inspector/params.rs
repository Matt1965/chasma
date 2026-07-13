//! Bundled resources for inspector capture (Bevy system param limit).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::debug::MovementBlockObservability;
use crate::simulation::SimulationControlState;
use crate::world::{
    BuildingCatalog, DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, UnitCatalog,
    WeaponCatalog, WorldConfig, WorldData,
};

/// Shared read-only inputs for inspector snapshot capture systems.
#[derive(SystemParam)]
pub struct InspectorCaptureParams<'w> {
    pub world: Res<'w, WorldData>,
    pub config: Res<'w, WorldConfig>,
    pub unit_catalog: Res<'w, UnitCatalog>,
    pub weapon_catalog: Res<'w, WeaponCatalog>,
    pub doodad_catalog: Res<'w, DoodadCatalog>,
    pub building_catalog: Res<'w, BuildingCatalog>,
    pub interior_catalog: Res<'w, InteriorProfileCatalog>,
    pub footprint_catalog: Res<'w, FootprintCatalog>,
    pub simulation: Res<'w, SimulationControlState>,
    pub movement_blocks: Res<'w, MovementBlockObservability>,
}
