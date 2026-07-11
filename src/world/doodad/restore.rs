//! Validated doodad restore for dev scenes (REVIEW-A5).

use std::collections::HashSet;

use bevy::prelude::Vec3;

use super::catalog::DoodadCatalog;
use super::id::DoodadId;
use super::procedural_key::ProceduralDoodadKey;
use super::record::DoodadRecord;
use super::source::DoodadSource;
use crate::world::{DoodadInsertError, DoodadKind, WorldData};

/// Why a doodad restore was rejected.
#[derive(Debug, Clone, PartialEq)]
pub enum DoodadRestoreError {
    MissingDoodadDefinition {
        doodad_id: u64,
        definition_id: String,
    },
    DisabledDoodadDefinition {
        doodad_id: u64,
        definition_id: String,
    },
    DefinitionKindMismatch {
        doodad_id: u64,
        expected: DoodadKind,
        found: DoodadKind,
    },
    DuplicateDoodadId {
        doodad_id: u64,
    },
    DuplicateProceduralKey {
        doodad_id: u64,
    },
    ScaleOutOfRange {
        doodad_id: u64,
        min: f32,
        max: f32,
    },
    PlacementChunkMismatch {
        doodad_id: u64,
    },
    IndexInsert {
        doodad_id: u64,
        error: DoodadInsertError,
    },
}

impl DoodadRestoreError {
    pub fn doodad_id(&self) -> u64 {
        match self {
            Self::MissingDoodadDefinition { doodad_id, .. }
            | Self::DisabledDoodadDefinition { doodad_id, .. }
            | Self::DefinitionKindMismatch { doodad_id, .. }
            | Self::DuplicateDoodadId { doodad_id }
            | Self::DuplicateProceduralKey { doodad_id }
            | Self::ScaleOutOfRange { doodad_id, .. }
            | Self::PlacementChunkMismatch { doodad_id }
            | Self::IndexInsert { doodad_id, .. } => *doodad_id,
        }
    }
}

fn validate_scale(
    min: f32,
    max: f32,
    scale: Vec3,
    doodad_id: u64,
) -> Result<(), DoodadRestoreError> {
    for component in [scale.x, scale.y, scale.z] {
        if component < min || component > max {
            return Err(DoodadRestoreError::ScaleOutOfRange {
                doodad_id,
                min,
                max,
            });
        }
    }
    Ok(())
}

fn validate_doodad_record_fields(
    catalog: &DoodadCatalog,
    record: &DoodadRecord,
    seen_ids: &HashSet<DoodadId>,
    seen_procedural_keys: &HashSet<ProceduralDoodadKey>,
) -> Result<(), DoodadRestoreError> {
    let doodad_id = record.id.raw();
    if seen_ids.contains(&record.id) {
        return Err(DoodadRestoreError::DuplicateDoodadId { doodad_id });
    }

    let definition_id = record.definition_id.clone();
    let Some(definition) = catalog.get(&definition_id) else {
        return Err(DoodadRestoreError::MissingDoodadDefinition {
            doodad_id,
            definition_id: definition_id.as_str().to_string(),
        });
    };
    if !definition.enabled {
        return Err(DoodadRestoreError::DisabledDoodadDefinition {
            doodad_id,
            definition_id: definition_id.as_str().to_string(),
        });
    }
    if record.kind != definition.kind {
        return Err(DoodadRestoreError::DefinitionKindMismatch {
            doodad_id,
            expected: definition.kind,
            found: record.kind,
        });
    }

    validate_scale(
        definition.min_scale,
        definition.max_scale,
        record.placement.scale,
        doodad_id,
    )?;

    let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
    if record.placement.position.chunk != chunk.coord() {
        return Err(DoodadRestoreError::PlacementChunkMismatch { doodad_id });
    }

    if let Some(key) = ProceduralDoodadKey::from_record(record) {
        if seen_procedural_keys.contains(&key) {
            return Err(DoodadRestoreError::DuplicateProceduralKey { doodad_id });
        }
    }

    Ok(())
}

/// Validate a doodad record without mutating [`WorldData`].
///
/// Does not consult existing world instances: scene load replaces all doodads after
/// validation, so only within-scene duplicate ids/keys are rejected here.
pub fn validate_doodad_for_restore(
    catalog: &DoodadCatalog,
    record: &DoodadRecord,
    seen_ids: &HashSet<DoodadId>,
    seen_procedural_keys: &HashSet<ProceduralDoodadKey>,
) -> Result<(), DoodadRestoreError> {
    validate_doodad_record_fields(catalog, record, seen_ids, seen_procedural_keys)
}

/// Insert a validated doodad record with preserved id (dev scene restore).
pub fn restore_doodad_record(
    world: &mut WorldData,
    catalog: &DoodadCatalog,
    record: DoodadRecord,
    seen_ids: &mut HashSet<DoodadId>,
    seen_procedural_keys: &mut HashSet<ProceduralDoodadKey>,
) -> Result<(), DoodadRestoreError> {
    validate_doodad_record_fields(catalog, &record, seen_ids, seen_procedural_keys)?;

    let doodad_id = record.id.raw();
    seen_ids.insert(record.id);
    if let Some(key) = ProceduralDoodadKey::from_record(&record) {
        if !seen_procedural_keys.insert(key) {
            return Err(DoodadRestoreError::DuplicateProceduralKey { doodad_id });
        }
    }

    let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
    world
        .insert_doodad(chunk, record.clone())
        .map_err(|error| DoodadRestoreError::IndexInsert { doodad_id, error })?;

    if matches!(record.source, DoodadSource::Procedural { .. }) {
        if let Some(key) = ProceduralDoodadKey::from_record(&record) {
            world.register_procedural_doodad(key, record.id);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::catalog::DoodadDefinitionId;
    use super::*;
    use crate::world::{ChunkCoord, DoodadPlacement, DoodadSource, LocalPosition, WorldPosition};
    use bevy::prelude::{Quat, Vec3};

    fn layout_world() -> WorldData {
        WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn catalog() -> DoodadCatalog {
        DoodadCatalog::default()
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn sample_record(id: u64) -> DoodadRecord {
        DoodadRecord::new(
            DoodadId::new(id),
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            DoodadPlacement::new(pos(1.0, 2.0), Quat::IDENTITY, Vec3::ONE),
            DoodadSource::Dev,
        )
    }

    #[test]
    fn restore_preserves_doodad_id() {
        let cat = catalog();
        let mut world = layout_world();
        let mut seen_ids = HashSet::new();
        let mut seen_keys = HashSet::new();
        restore_doodad_record(
            &mut world,
            &cat,
            sample_record(99),
            &mut seen_ids,
            &mut seen_keys,
        )
        .unwrap();
        assert!(world.get_doodad(DoodadId::new(99)).is_some());
    }

    #[test]
    fn duplicate_doodad_id_rejected() {
        let cat = catalog();
        let mut world = layout_world();
        let mut seen_ids = HashSet::new();
        let mut seen_keys = HashSet::new();
        restore_doodad_record(
            &mut world,
            &cat,
            sample_record(1),
            &mut seen_ids,
            &mut seen_keys,
        )
        .unwrap();
        let err = restore_doodad_record(
            &mut world,
            &cat,
            sample_record(1),
            &mut seen_ids,
            &mut seen_keys,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DoodadRestoreError::DuplicateDoodadId { doodad_id: 1 }
        ));
    }
}
