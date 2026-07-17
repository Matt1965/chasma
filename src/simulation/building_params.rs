//! Bundled terrain/operation resources for simulation ticks (ADR-105 TF5).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::world::{
    BuildingFieldRequirementCatalog, BuildingFieldRequirementCatalogRevision,
    BuildingOperationStore, BuildingTerrainAssessmentStore, FieldResponseProfileCatalog,
    FieldResponseProfileCatalogRevision, TerrainFieldCatalog,
};

/// Terrain assessment and operation stores for authoritative simulation (ADR-105 TF5).
#[derive(SystemParam)]
pub struct BuildingSimulationParams<'w> {
    pub field_catalog: Res<'w, TerrainFieldCatalog>,
    pub requirement_catalog: Res<'w, BuildingFieldRequirementCatalog>,
    pub profile_catalog: Res<'w, FieldResponseProfileCatalog>,
    pub requirement_revision: Res<'w, BuildingFieldRequirementCatalogRevision>,
    pub profile_revision: Res<'w, FieldResponseProfileCatalogRevision>,
    pub assessment_store: ResMut<'w, BuildingTerrainAssessmentStore>,
    pub operation_store: ResMut<'w, BuildingOperationStore>,
}

impl BuildingSimulationParams<'_> {
    pub fn operation_params<'a>(
        &'a mut self,
        building_catalog: &'a crate::world::BuildingCatalog,
        footprint_catalog: &'a crate::world::FootprintCatalog,
    ) -> crate::world::BuildingOperationParams<'a> {
        crate::world::BuildingOperationParams {
            field_catalog: &self.field_catalog,
            requirement_catalog: &self.requirement_catalog,
            profile_catalog: &self.profile_catalog,
            footprint_catalog,
            requirement_revision: self.requirement_revision.0,
            profile_revision: self.profile_revision.0,
            assessment_store: &mut self.assessment_store,
            operation_store: &mut self.operation_store,
        }
    }
}
