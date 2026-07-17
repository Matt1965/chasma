//! Bundled resources for inspector capture (Bevy system param limit).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
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

/// World picking queries for inspector input (Bevy system param limit).
#[derive(SystemParam)]
pub struct InspectorPickParams<'w, 's> {
    pub windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<RtsCamera>>,
    pub units: Query<
        'w,
        's,
        (
            &'static crate::units::UnitRenderEntity,
            &'static GlobalTransform,
        ),
    >,
    pub buildings: Query<
        'w,
        's,
        (
            &'static crate::buildings::components::BuildingRenderEntity,
            &'static GlobalTransform,
        ),
    >,
    pub doodads: Query<
        'w,
        's,
        (
            &'static crate::doodads::components::DoodadRenderEntity,
            &'static GlobalTransform,
        ),
    >,
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
