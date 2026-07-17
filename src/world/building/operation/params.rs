use crate::world::building::field_requirement::BuildingFieldRequirementCatalog;
use crate::world::building::field_response::FieldResponseProfileCatalog;
use crate::world::building::terrain_assessment::BuildingTerrainAssessmentStore;
use crate::world::{BuildingCatalog, FootprintCatalog, TerrainFieldCatalog, WorldData};

use super::store::BuildingOperationStore;

/// Catalog bundle for workstation operation stepping (ADR-105 TF5).
pub struct BuildingOperationParams<'a> {
    pub field_catalog: &'a TerrainFieldCatalog,
    pub requirement_catalog: &'a BuildingFieldRequirementCatalog,
    pub profile_catalog: &'a FieldResponseProfileCatalog,
    pub footprint_catalog: &'a FootprintCatalog,
    pub requirement_revision: u64,
    pub profile_revision: u64,
    pub assessment_store: &'a mut BuildingTerrainAssessmentStore,
    pub operation_store: &'a mut BuildingOperationStore,
}

impl<'a> BuildingOperationParams<'a> {
    pub fn terrain_catalogs<'b>(
        &'b self,
        building_catalog: &'b BuildingCatalog,
    ) -> crate::world::building::terrain_assessment::TerrainAssessmentCatalogs<'b>
    where
        'a: 'b,
    {
        crate::world::building::terrain_assessment::TerrainAssessmentCatalogs {
            buildings: building_catalog,
            requirements: self.requirement_catalog,
            profiles: self.profile_catalog,
            fields: self.field_catalog,
            footprints: self.footprint_catalog,
            requirement_revision: self.requirement_revision,
            profile_revision: self.profile_revision,
        }
    }

    pub fn efficiency_context<'b>(
        &'b mut self,
        world: &'b WorldData,
        building_catalog: &'b BuildingCatalog,
    ) -> crate::world::building::operational_efficiency::OperationalEfficiencyContext<'b>
    where
        'a: 'b,
    {
        let terrain_catalogs =
            crate::world::building::terrain_assessment::TerrainAssessmentCatalogs {
                buildings: building_catalog,
                requirements: self.requirement_catalog,
                profiles: self.profile_catalog,
                fields: self.field_catalog,
                footprints: self.footprint_catalog,
                requirement_revision: self.requirement_revision,
                profile_revision: self.profile_revision,
            };
        crate::world::building::operational_efficiency::OperationalEfficiencyContext {
            world,
            building_catalog,
            terrain_catalogs,
            assessment_store: self.assessment_store,
        }
    }
}
