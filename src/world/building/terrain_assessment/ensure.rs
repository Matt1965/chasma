//! Ensure cached terrain assessments are present and fresh (ADR-104/105, ADR-106 TF6).

use crate::world::BuildingId;
use crate::world::building::record::BuildingRecord;
use crate::world::building::terrain_assessment::{
    BuildingTerrainAssessment, BuildingTerrainAssessmentKey, BuildingTerrainAssessmentStore,
    TerrainAssessmentCatalogs, assess_building_terrain,
};

/// Assessment revision fingerprint for operational-efficiency linkage.
pub fn assessment_revision_fingerprint(assessment: &BuildingTerrainAssessment) -> u64 {
    let mut hash = assessment.sample_footprint_hash;
    hash ^= assessment.requirement_catalog_revision.rotate_left(7);
    hash ^= assessment.profile_catalog_revision.rotate_left(13);
    for entry in &assessment.field_tile_revisions {
        hash ^= entry.tile_revision.rotate_left(3);
        hash ^= entry.chunk.x as u64;
        hash ^= (entry.chunk.z as u64) << 16;
    }
    hash
}

/// Return cached assessment or recompute and store when missing/stale.
pub fn ensure_building_terrain_assessment(
    world: &crate::world::WorldData,
    catalogs: &TerrainAssessmentCatalogs<'_>,
    store: &mut BuildingTerrainAssessmentStore,
    building_id: BuildingId,
    record: &BuildingRecord,
) -> BuildingTerrainAssessment {
    if !store.is_dirty(building_id) {
        if let (Some(cached), Some(stored_key)) =
            (store.get(building_id), store.stored_key(building_id))
        {
            if !cached.stale {
                let probe = assess_building_terrain(world, catalogs, record, world.layout());
                let fresh_key =
                    BuildingTerrainAssessmentKey::from_assessment(record.placement, &probe);
                if stored_key == &fresh_key {
                    return cached.clone();
                }
                store.insert(building_id, fresh_key, probe);
                return store.get(building_id).cloned().unwrap();
            }
        }
    }

    let assessment = assess_building_terrain(world, catalogs, record, world.layout());
    let key = BuildingTerrainAssessmentKey::from_assessment(record.placement, &assessment);
    store.insert(building_id, key, assessment.clone());
    assessment
}
