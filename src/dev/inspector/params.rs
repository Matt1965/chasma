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

/// Runtime building presentation inputs for dev inspector asset diagnostics (ADR-095 BA1).
#[derive(SystemParam)]
pub struct BuildingInspectorPresentationParams<'w, 's> {
    pub asset_server: Res<'w, AssetServer>,
    pub scene_assets: Res<'w, crate::buildings::BuildingSceneAssets>,
    pub render_index: Res<'w, crate::buildings::BuildingRenderIndex>,
    pub render_entities: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::buildings::BuildingRenderEntity,
            Option<&'static crate::buildings::BuildingDiagnosticFallback>,
            Option<&'static crate::buildings::BuildingSceneTags>,
        ),
    >,
}
