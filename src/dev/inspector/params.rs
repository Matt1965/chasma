//! Bundled resources for inspector capture (Bevy system param limit).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::debug::MovementBlockObservability;
use crate::simulation::{BuildingSimulationParams, SimulationControlState};
use crate::world::{
    BuildingCatalog, BuildingFieldRequirementCatalog, BuildingFieldRequirementCatalogRevision,
    BuildingInteractionProfileCatalog, BuildingTerrainAssessmentStore, DoodadCatalog,
    FieldResponseProfileCatalog, FieldResponseProfileCatalogRevision, FootprintCatalog,
    InteriorProfileCatalog, InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, ItemPileSettings, OperationCatalog, TerrainFieldCatalog, UnitCatalog,
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
    pub operation_catalog: Res<'w, OperationCatalog>,
    pub items: Res<'w, ItemCatalog>,
    pub item_categories: Res<'w, ItemCategoryCatalog>,
    pub inventory_profiles: Res<'w, InventoryProfileCatalog>,
    pub field_catalog: Res<'w, TerrainFieldCatalog>,
    pub requirements: Res<'w, BuildingFieldRequirementCatalog>,
    pub profile_catalog: Res<'w, FieldResponseProfileCatalog>,
    pub requirement_revision: Res<'w, BuildingFieldRequirementCatalogRevision>,
    pub profile_revision: Res<'w, FieldResponseProfileCatalogRevision>,
    pub assessments: ResMut<'w, BuildingTerrainAssessmentStore>,
    pub simulation: Res<'w, SimulationControlState>,
    pub movement_blocks: Res<'w, MovementBlockObservability>,
}

/// Bundled catalogs for building dev shortcuts (Bevy 16-param system limit, EP9).
#[derive(SystemParam)]
pub struct DevBuildingActionParams<'w> {
    pub doodad_catalog: Res<'w, DoodadCatalog>,
    pub building_catalog: Res<'w, BuildingCatalog>,
    pub footprint_catalog: Res<'w, FootprintCatalog>,
    pub interior_catalog: Res<'w, InteriorProfileCatalog>,
    pub interaction_catalog: Res<'w, BuildingInteractionProfileCatalog>,
    pub items: Res<'w, ItemCatalog>,
    pub item_categories: Res<'w, ItemCategoryCatalog>,
    pub inventory_profiles: Res<'w, InventoryProfileCatalog>,
    pub pile_settings: Res<'w, ItemPileSettings>,
}

impl DevBuildingActionParams<'_> {
    pub fn inventory_ctx(&self) -> InventoryCatalogCtx<'_> {
        InventoryCatalogCtx::new(&self.items, &self.item_categories, &self.inventory_profiles)
    }
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
