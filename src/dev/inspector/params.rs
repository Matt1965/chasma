//! Bundled resources for inspector capture (Bevy system param limit).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::simulation::SimulationControlState;
use crate::world::{DoodadCatalog, UnitCatalog, WorldConfig, WorldData};

/// Shared read-only inputs for inspector snapshot capture systems.
#[derive(SystemParam)]
pub struct InspectorCaptureParams<'w> {
    pub world: Res<'w, WorldData>,
    pub config: Res<'w, WorldConfig>,
    pub unit_catalog: Res<'w, UnitCatalog>,
    pub doodad_catalog: Res<'w, DoodadCatalog>,
    pub simulation: Res<'w, SimulationControlState>,
}
