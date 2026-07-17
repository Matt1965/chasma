use super::types::BuildingTerrainAssessment;
use crate::world::building::placement::BuildingPlacement;

/// Cache validity key for terrain assessments (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingTerrainAssessmentKey {
    pub sample_footprint_hash: u64,
    pub requirement_catalog_revision: u64,
    pub profile_catalog_revision: u64,
    pub placement_fingerprint: u64,
    pub field_tile_revisions_fingerprint: u64,
}

impl BuildingTerrainAssessmentKey {
    pub fn from_assessment(
        placement: BuildingPlacement,
        assessment: &BuildingTerrainAssessment,
    ) -> Self {
        let field_tile_revisions_fingerprint =
            fingerprint_tile_revisions(&assessment.field_tile_revisions);
        Self {
            sample_footprint_hash: assessment.sample_footprint_hash,
            requirement_catalog_revision: assessment.requirement_catalog_revision,
            profile_catalog_revision: assessment.profile_catalog_revision,
            placement_fingerprint: fingerprint_placement(&placement),
            field_tile_revisions_fingerprint,
        }
    }
}

fn fingerprint_placement(placement: &BuildingPlacement) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    placement.position.chunk.x.hash(&mut hasher);
    placement.position.chunk.z.hash(&mut hasher);
    placement.position.local.0.x.to_bits().hash(&mut hasher);
    placement.position.local.0.z.to_bits().hash(&mut hasher);
    placement.rotation.x.to_bits().hash(&mut hasher);
    placement.rotation.y.to_bits().hash(&mut hasher);
    placement.rotation.z.to_bits().hash(&mut hasher);
    placement.rotation.w.to_bits().hash(&mut hasher);
    placement.uniform_scale.0.hash(&mut hasher);
    hasher.finish()
}

fn fingerprint_tile_revisions(entries: &[super::types::FieldTileRevisionEntry]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for entry in entries {
        entry.field_id.as_str().hash(&mut hasher);
        entry.chunk.x.hash(&mut hasher);
        entry.chunk.z.hash(&mut hasher);
        entry.tile_revision.hash(&mut hasher);
    }
    hasher.finish()
}

/// Deterministic hash of sorted sample cells.
pub fn hash_sample_cells(cells: &[crate::world::OccupancyCellCoord]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cells.len().hash(&mut hasher);
    for cell in cells {
        cell.x.hash(&mut hasher);
        cell.z.hash(&mut hasher);
    }
    hasher.finish()
}
