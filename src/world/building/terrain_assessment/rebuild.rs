//! Authoritative assessment rebuild entry point (ADR-106 TF6).

use crate::world::BuildingId;
use crate::world::building::catalog::BuildingDefinitionId;
use crate::world::building::record::BuildingRecord;

use super::assess::assess_building_terrain;
use super::error::TerrainAssessmentCatalogs;
use super::revision::BuildingTerrainAssessmentKey;
use super::store::BuildingTerrainAssessmentStore;

/// Per-building outcome from a rebuild pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssessmentRebuildOutcome {
    Assessed,
    SkippedNoRequirements,
    SkippedMissingBuilding,
    Failed(String),
}

/// Summary of [`rebuild_all_building_terrain_assessments`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssessmentRebuildReport {
    pub assessed: usize,
    pub skipped_no_requirements: usize,
    pub skipped_missing: usize,
    pub failures: Vec<(BuildingId, String)>,
}

impl AssessmentRebuildReport {
    pub fn record(&mut self, building_id: BuildingId, outcome: AssessmentRebuildOutcome) {
        match outcome {
            AssessmentRebuildOutcome::Assessed => self.assessed += 1,
            AssessmentRebuildOutcome::SkippedNoRequirements => {
                self.skipped_no_requirements += 1;
            }
            AssessmentRebuildOutcome::SkippedMissingBuilding => self.skipped_missing += 1,
            AssessmentRebuildOutcome::Failed(message) => self.failures.push((building_id, message)),
        }
    }
}

/// Rebuild cached terrain assessments for every placed building (ADR-106 TF6).
///
/// Deterministic building order. Per-building failures do not abort the batch.
pub fn rebuild_all_building_terrain_assessments(
    world: &crate::world::WorldData,
    catalogs: &TerrainAssessmentCatalogs<'_>,
    store: &mut BuildingTerrainAssessmentStore,
) -> AssessmentRebuildReport {
    let mut report = AssessmentRebuildReport::default();
    for building_id in world.sorted_building_ids() {
        let outcome = rebuild_building_terrain_assessment(world, catalogs, store, building_id);
        report.record(building_id, outcome);
    }
    report
}

/// Rebuild one building assessment; returns structured outcome.
pub fn rebuild_building_terrain_assessment(
    world: &crate::world::WorldData,
    catalogs: &TerrainAssessmentCatalogs<'_>,
    store: &mut BuildingTerrainAssessmentStore,
    building_id: BuildingId,
) -> AssessmentRebuildOutcome {
    let Some(record) = world.get_building(building_id).cloned() else {
        store.remove(building_id);
        return AssessmentRebuildOutcome::SkippedMissingBuilding;
    };
    rebuild_building_terrain_assessment_for_record(world, catalogs, store, building_id, &record)
}

fn rebuild_building_terrain_assessment_for_record(
    world: &crate::world::WorldData,
    catalogs: &TerrainAssessmentCatalogs<'_>,
    store: &mut BuildingTerrainAssessmentStore,
    building_id: BuildingId,
    record: &BuildingRecord,
) -> AssessmentRebuildOutcome {
    if catalogs
        .requirements
        .active_required_efficiency(&record.definition_id)
        .is_empty()
    {
        store.remove(building_id);
        return AssessmentRebuildOutcome::SkippedNoRequirements;
    }
    let assessment = assess_building_terrain(world, catalogs, record, world.layout());
    let key = BuildingTerrainAssessmentKey::from_assessment(record.placement, &assessment);
    store.insert(building_id, key, assessment);
    AssessmentRebuildOutcome::Assessed
}

/// Mark assessments dirty for buildings that reference any changed field (ADR-106 TF6).
pub fn invalidate_buildings_for_changed_fields(
    world: &crate::world::WorldData,
    catalogs: &TerrainAssessmentCatalogs<'_>,
    store: &mut BuildingTerrainAssessmentStore,
    changed_fields: &[crate::world::TerrainFieldId],
) -> usize {
    if changed_fields.is_empty() {
        return 0;
    }
    let mut invalidated = 0usize;
    for building_id in world.sorted_building_ids() {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if building_references_any_field(catalogs, &record.definition_id, changed_fields) {
            store.mark_dirty(building_id);
            invalidated += 1;
        }
    }
    invalidated
}

fn building_references_any_field(
    catalogs: &TerrainAssessmentCatalogs<'_>,
    definition_id: &BuildingDefinitionId,
    changed_fields: &[crate::world::TerrainFieldId],
) -> bool {
    catalogs
        .requirements
        .active_required_efficiency(definition_id)
        .iter()
        .any(|requirement| {
            changed_fields
                .iter()
                .any(|field| field == &requirement.terrain_field_id)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::terrain_assessment::BuildingTerrainAssessmentStore;
    use crate::world::{
        BuildingCategoryCatalog, BuildingDefinition, BuildingDefinitionId, BuildingLifecycleState,
        BuildingOwnership, BuildingPlacement, BuildingRecord, BuildingRenderKey, BuildingSource,
        ChunkCoord, ChunkExtent, ChunkId, FootprintCatalog, FootprintSpec, LocalPosition,
        WorldData, WorldPosition,
    };
    use bevy::prelude::{Quat, Vec3};

    fn setup() -> (WorldData, BuildingId, TerrainAssessmentCatalogs<'static>) {
        let layout = crate::world::WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        crate::world::bootstrap_constant_field(
            world.terrain_fields_mut(),
            crate::world::TerrainFieldId::new("iron"),
            ChunkCoord::new(0, 0),
            crate::world::field_value_from_percent(50.0),
        );
        let building_id = world.allocate_building_id();
        let mut record = BuildingRecord::new(
            building_id,
            BuildingDefinitionId::new("iron_mine"),
            BuildingPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
                ),
                Quat::IDENTITY,
            ),
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
            400,
            BuildingSource::Authored,
        );
        record.lifecycle_state = BuildingLifecycleState::Complete;
        record.construction.progress_0_1 = 1.0;
        world
            .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();

        let field_catalog = crate::world::TerrainFieldCatalog::default();
        let profile_catalog = crate::world::FieldResponseProfileCatalog::default();
        let requirement_catalog = crate::world::BuildingFieldRequirementCatalog::default();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog = crate::world::BuildingCatalog::from_definitions(
            vec![BuildingDefinition::new(
                BuildingDefinitionId::new("iron_mine"),
                "Iron Mine",
                crate::world::BuildingCategoryId::new("production"),
                BuildingRenderKey::reserved("smelter"),
                BuildingRenderKey::reserved("smelter_collision"),
                400,
                90.0,
                FootprintSpec::Circle { radius_meters: 2.5 },
                30.0,
                true,
            )],
            &categories,
        )
        .unwrap();
        let footprint_catalog = FootprintCatalog::default();
        let catalogs = TerrainAssessmentCatalogs {
            buildings: Box::leak(Box::new(building_catalog)),
            requirements: Box::leak(Box::new(requirement_catalog)),
            profiles: Box::leak(Box::new(profile_catalog)),
            fields: Box::leak(Box::new(field_catalog)),
            footprints: Box::leak(Box::new(footprint_catalog)),
            requirement_revision: 0,
            profile_revision: 0,
        };
        (world, building_id, catalogs)
    }

    #[test]
    fn rebuild_all_is_deterministic() {
        let (world, building_id, catalogs) = setup();
        let mut store_a = BuildingTerrainAssessmentStore::default();
        let mut store_b = BuildingTerrainAssessmentStore::default();
        let report_a = rebuild_all_building_terrain_assessments(&world, &catalogs, &mut store_a);
        let report_b = rebuild_all_building_terrain_assessments(&world, &catalogs, &mut store_b);
        assert_eq!(report_a, report_b);
        assert_eq!(report_a.assessed, 1);
        assert_eq!(store_a.get(building_id), store_b.get(building_id));
    }
}
