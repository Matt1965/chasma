//! Bundled terrain/operation resources for simulation ticks (ADR-105 TF5, EP1/EP3).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::world::{
    BuildingFieldRequirementCatalog, BuildingFieldRequirementCatalogRevision,
    BuildingTerrainAssessmentStore, FieldResponseProfileCatalog,
    FieldResponseProfileCatalogRevision, OperationCatalog, TerrainFieldCatalog,
};

/// Terrain assessment store for authoritative simulation (ADR-105 TF5, EP1).
#[derive(SystemParam)]
pub struct BuildingSimulationParams<'w> {
    pub field_catalog: Res<'w, TerrainFieldCatalog>,
    pub requirement_catalog: Res<'w, BuildingFieldRequirementCatalog>,
    pub profile_catalog: Res<'w, FieldResponseProfileCatalog>,
    pub operation_catalog: Res<'w, OperationCatalog>,
    pub requirement_revision: Res<'w, BuildingFieldRequirementCatalogRevision>,
    pub profile_revision: Res<'w, FieldResponseProfileCatalogRevision>,
    pub assessment_store: ResMut<'w, BuildingTerrainAssessmentStore>,
}

impl BuildingSimulationParams<'_> {
    pub fn operation_params<'a>(
        &'a mut self,
        _building_catalog: &'a crate::world::BuildingCatalog,
        footprint_catalog: &'a crate::world::FootprintCatalog,
        inventory_ctx: &'a crate::world::InventoryCatalogCtx<'a>,
    ) -> crate::world::BuildingOperationParams<'a> {
        crate::world::BuildingOperationParams {
            field_catalog: &self.field_catalog,
            requirement_catalog: &self.requirement_catalog,
            profile_catalog: &self.profile_catalog,
            footprint_catalog,
            operation_catalog: &self.operation_catalog,
            inventory_ctx,
            requirement_revision: self.requirement_revision.0,
            profile_revision: self.profile_revision.0,
            assessment_store: &mut self.assessment_store,
        }
    }
}
