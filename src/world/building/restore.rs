//! Building instance restore validation (ADR-086 B9).

use std::collections::HashSet;

use bevy::prelude::*;

use super::catalog::{BuildingCatalog, BuildingDefinitionId};
use super::record::BuildingRecord;
use crate::world::BuildingId;

/// Why a building record cannot be restored.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildingRestoreError {
    DuplicateId(BuildingId),
    MissingDefinition(BuildingDefinitionId),
    DisabledDefinition(BuildingDefinitionId),
    InvalidProgress(f32),
    InvalidVitals { current_hp: u32, max_hp: u32 },
    InvalidLifecycle(&'static str),
}

impl std::fmt::Display for BuildingRestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate building id {id:?}"),
            Self::MissingDefinition(id) => write!(f, "missing definition {id}"),
            Self::DisabledDefinition(id) => write!(f, "disabled definition {id}"),
            Self::InvalidProgress(value) => write!(f, "invalid progress {value}"),
            Self::InvalidVitals { current_hp, max_hp } => {
                write!(f, "invalid vitals {current_hp}/{max_hp}")
            }
            Self::InvalidLifecycle(reason) => write!(f, "invalid lifecycle: {reason}"),
        }
    }
}

/// Validate a building record before scene restore (REVIEW-A5 parity).
pub fn validate_building_for_restore(
    catalog: &BuildingCatalog,
    record: &BuildingRecord,
    seen_ids: &HashSet<BuildingId>,
) -> Result<(), BuildingRestoreError> {
    if seen_ids.contains(&record.id) {
        return Err(BuildingRestoreError::DuplicateId(record.id));
    }
    let definition = catalog
        .get(&record.definition_id)
        .ok_or_else(|| BuildingRestoreError::MissingDefinition(record.definition_id.clone()))?;
    if !definition.enabled {
        return Err(BuildingRestoreError::DisabledDefinition(
            record.definition_id.clone(),
        ));
    }
    if !record.construction.progress_0_1.is_finite()
        || !(0.0..=1.0).contains(&record.construction.progress_0_1)
    {
        return Err(BuildingRestoreError::InvalidProgress(
            record.construction.progress_0_1,
        ));
    }
    if record.vitals.max_hp == 0 || record.vitals.current_hp > record.vitals.max_hp {
        return Err(BuildingRestoreError::InvalidVitals {
            current_hp: record.vitals.current_hp,
            max_hp: record.vitals.max_hp,
        });
    }
    if record.lifecycle_state.label().is_empty() {
        return Err(BuildingRestoreError::InvalidLifecycle("empty lifecycle"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingOwnership, BuildingPlacement, BuildingSource, BuildingVitals, ChunkCoord,
        ChunkLayout, LocalPosition, WorldPosition,
    };

    fn sample_record() -> BuildingRecord {
        BuildingRecord::new(
            BuildingId::new(1),
            BuildingDefinitionId::new("hut"),
            BuildingPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
                ),
                Quat::IDENTITY,
            ),
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
            100,
            BuildingSource::Authored,
        )
    }

    #[test]
    fn valid_building_passes_validation() {
        let catalog = BuildingCatalog::default();
        let record = sample_record();
        assert!(validate_building_for_restore(&catalog, &record, &HashSet::new()).is_ok());
    }

    #[test]
    fn duplicate_id_rejected() {
        let catalog = BuildingCatalog::default();
        let record = sample_record();
        let mut seen = HashSet::from([record.id]);
        assert!(matches!(
            validate_building_for_restore(&catalog, &record, &seen),
            Err(BuildingRestoreError::DuplicateId(_))
        ));
    }
}
