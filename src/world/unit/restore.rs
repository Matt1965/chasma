//! Validated unit restore for dev scenes (REVIEW-A5).
//!
//! Scene restore preserves snapshot ids and placement but normalizes vitals and
//! clears ephemeral combat timing state.

use std::collections::HashSet;

use super::catalog::UnitCatalog;
use super::combat_state::CombatState;
use super::id::UnitId;
use super::record::UnitRecord;
use super::state::UnitState;
use crate::world::{UnitInsertError, WorldData};

/// Why a unit restore was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitRestoreError {
    MissingUnitDefinition {
        unit_id: u64,
        definition_id: String,
    },
    DisabledUnitDefinition {
        unit_id: u64,
        definition_id: String,
    },
    DuplicateUnitId {
        unit_id: u64,
    },
    PlacementChunkMismatch {
        unit_id: u64,
    },
    InvalidVitals {
        unit_id: u64,
        reason: &'static str,
    },
    IndexInsert {
        unit_id: u64,
        error: UnitInsertError,
    },
}

impl UnitRestoreError {
    pub fn unit_id(&self) -> u64 {
        match self {
            Self::MissingUnitDefinition { unit_id, .. }
            | Self::DisabledUnitDefinition { unit_id, .. }
            | Self::DuplicateUnitId { unit_id }
            | Self::PlacementChunkMismatch { unit_id }
            | Self::InvalidVitals { unit_id, .. }
            | Self::IndexInsert { unit_id, .. } => *unit_id,
        }
    }
}

/// Normalize persistent placement/ownership and reset ephemeral combat state.
pub fn normalize_restored_unit(
    record: &mut UnitRecord,
    catalog: &UnitCatalog,
) -> Result<(), UnitRestoreError> {
    let unit_id = record.id.raw();
    let definition_id = record.definition_id.clone();
    let Some(definition) = catalog.get(&definition_id) else {
        return Err(UnitRestoreError::MissingUnitDefinition {
            unit_id,
            definition_id: definition_id.as_str().to_string(),
        });
    };
    if !definition.enabled {
        return Err(UnitRestoreError::DisabledUnitDefinition {
            unit_id,
            definition_id: definition_id.as_str().to_string(),
        });
    }

    record.vitals.max_hp = definition.max_hp;
    match record.state {
        UnitState::Dead => {
            record.vitals.current_hp = 0;
        }
        _ => {
            record.vitals.current_hp = definition.max_hp;
        }
    }

    if record.vitals.max_hp == 0 {
        return Err(UnitRestoreError::InvalidVitals {
            unit_id,
            reason: "max_hp must be greater than zero",
        });
    }
    if record.vitals.current_hp > record.vitals.max_hp {
        return Err(UnitRestoreError::InvalidVitals {
            unit_id,
            reason: "current_hp exceeds max_hp",
        });
    }

    record.combat_state = CombatState::Peaceful;
    record.attack_cycle = None;
    Ok(())
}

/// Validate a unit record without mutating [`WorldData`].
///
/// Does not consult existing world instances: scene load replaces all units after
/// validation, so only within-scene duplicate ids are rejected here.
pub fn validate_unit_for_restore(
    catalog: &UnitCatalog,
    record: &UnitRecord,
    seen_ids: &HashSet<UnitId>,
) -> Result<(), UnitRestoreError> {
    let unit_id = record.id.raw();
    if seen_ids.contains(&record.id) {
        return Err(UnitRestoreError::DuplicateUnitId { unit_id });
    }
    let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
    if record.placement.position.chunk != chunk.coord() {
        return Err(UnitRestoreError::PlacementChunkMismatch { unit_id });
    }
    let mut normalized = record.clone();
    normalize_restored_unit(&mut normalized, catalog)?;
    Ok(())
}

/// Insert a validated unit record with preserved id (dev scene restore).
pub fn restore_unit_record(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    mut record: UnitRecord,
    seen_ids: &mut HashSet<UnitId>,
) -> Result<(), UnitRestoreError> {
    let unit_id = record.id.raw();
    if !seen_ids.insert(record.id) {
        return Err(UnitRestoreError::DuplicateUnitId { unit_id });
    }
    if world.get_unit(record.id).is_some() {
        return Err(UnitRestoreError::DuplicateUnitId { unit_id });
    }

    normalize_restored_unit(&mut record, catalog)?;

    let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
    world
        .insert_unit(chunk, record)
        .map_err(|error| UnitRestoreError::IndexInsert { unit_id, error })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::UnitDefinitionId;
    use crate::world::ownership::UnitOwnership;
    use crate::world::{ChunkCoord, LocalPosition, UnitSource, WorldPosition};
    use bevy::prelude::{Quat, Vec3};

    fn layout_world() -> WorldData {
        WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn catalog() -> UnitCatalog {
        UnitCatalog::default()
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn sample_record(id: u64) -> UnitRecord {
        UnitRecord::new(
            UnitId::new(id),
            UnitDefinitionId::new("wolf"),
            crate::world::UnitPlacement::new(pos(1.0, 2.0), Quat::IDENTITY),
            UnitSource::Dev,
            UnitOwnership::hostile(),
            5,
        )
    }

    #[test]
    fn restore_preserves_id_and_ownership() {
        let cat = catalog();
        let mut world = layout_world();
        let mut record = sample_record(42);
        record.owner_id = UnitOwnership::player_default().owner_id;
        record.team_id = UnitOwnership::player_default().team_id;
        record.affiliation = UnitOwnership::player_default().affiliation;
        let mut seen = HashSet::new();
        restore_unit_record(&mut world, &cat, record.clone(), &mut seen).unwrap();
        let restored = world.get_unit(UnitId::new(42)).unwrap();
        assert_eq!(restored.owner_id, record.owner_id);
        assert_eq!(restored.team_id, record.team_id);
        assert_eq!(restored.affiliation, record.affiliation);
    }

    #[test]
    fn restore_normalizes_vitals_from_catalog() {
        let cat = catalog();
        let mut world = layout_world();
        let mut record = sample_record(1);
        record.vitals.current_hp = 1;
        record.vitals.max_hp = 1;
        let mut seen = HashSet::new();
        restore_unit_record(&mut world, &cat, record, &mut seen).unwrap();
        let restored = world.get_unit(UnitId::new(1)).unwrap();
        assert_eq!(
            restored.vitals.max_hp,
            cat.get(&UnitDefinitionId::new("wolf")).unwrap().max_hp
        );
        assert_eq!(restored.vitals.current_hp, restored.vitals.max_hp);
    }

    #[test]
    fn duplicate_unit_id_rejected() {
        let cat = catalog();
        let mut world = layout_world();
        let mut seen = HashSet::new();
        restore_unit_record(&mut world, &cat, sample_record(1), &mut seen).unwrap();
        let err = restore_unit_record(&mut world, &cat, sample_record(1), &mut seen).unwrap_err();
        assert!(matches!(
            err,
            UnitRestoreError::DuplicateUnitId { unit_id: 1 }
        ));
    }
}
