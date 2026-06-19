//! Authoritative doodad placement API (ADR-017, Phase 3C).
//!
//! Operates on [`crate::world::WorldData`] and [`super::catalog::DoodadCatalog`].
//! No ECS entities, rendering, terrain validation, or save/load.

use bevy::prelude::*;

use super::catalog::DoodadCatalog;
use super::id::DoodadId;
use super::placement::DoodadPlacement;
use super::record::DoodadRecord;
use super::source::DoodadSource;
use crate::world::{DoodadDefinitionId, DoodadInsertError, WorldData, WorldPosition};

/// Optional pose overrides when creating a doodad from a catalog definition.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DoodadPlacementOverrides {
    pub rotation: Option<Quat>,
    pub scale: Option<Vec3>,
}

/// Why an authoring operation failed (ADR-017).
#[derive(Debug, Clone, PartialEq)]
pub enum DoodadAuthoringError {
    DefinitionNotFound(DoodadDefinitionId),
    DefinitionDisabled(DoodadDefinitionId),
    DoodadNotFound(DoodadId),
    ScaleOutOfRange {
        min: f32,
        max: f32,
        scale: Vec3,
    },
    ChunkPlacementMismatch,
}

/// Create a doodad instance from a catalog definition and insert it into world data.
pub fn create_doodad(
    catalog: &DoodadCatalog,
    world: &mut WorldData,
    definition_id: &DoodadDefinitionId,
    position: WorldPosition,
    source: DoodadSource,
    overrides: DoodadPlacementOverrides,
) -> Result<DoodadRecord, DoodadAuthoringError> {
    let definition = catalog
        .get(definition_id)
        .ok_or_else(|| DoodadAuthoringError::DefinitionNotFound(definition_id.clone()))?;

    if !definition.enabled {
        return Err(DoodadAuthoringError::DefinitionDisabled(
            definition_id.clone(),
        ));
    }

    let rotation = overrides.rotation.unwrap_or(Quat::IDENTITY);
    let scale = overrides.scale.unwrap_or(Vec3::ONE);
    validate_scale(definition.min_scale, definition.max_scale, scale)?;

    let id = world.allocate_doodad_id();
    let record = DoodadRecord::new(
        id,
        definition.id.clone(),
        definition.kind,
        DoodadPlacement::new(position, rotation, scale),
        source,
    );

    let chunk = crate::world::ChunkId::new(position.chunk);
    world
        .insert_doodad(chunk, record.clone())
        .map_err(|error| match error {
            DoodadInsertError::ChunkPlacementMismatch => DoodadAuthoringError::ChunkPlacementMismatch,
            DoodadInsertError::DoodadNotFound => DoodadAuthoringError::DoodadNotFound(id),
        })?;

    Ok(record)
}

/// Move an existing doodad to a new world position, including cross-chunk moves.
pub fn move_doodad(
    world: &mut WorldData,
    id: DoodadId,
    new_position: WorldPosition,
) -> Result<DoodadRecord, DoodadAuthoringError> {
    world
        .relocate_doodad(id, new_position)
        .map_err(|error| match error {
            DoodadInsertError::ChunkPlacementMismatch => DoodadAuthoringError::ChunkPlacementMismatch,
            DoodadInsertError::DoodadNotFound => DoodadAuthoringError::DoodadNotFound(id),
        })
}

/// Remove a doodad by id, returning the removed record.
pub fn remove_doodad(
    world: &mut WorldData,
    id: DoodadId,
) -> Result<DoodadRecord, DoodadAuthoringError> {
    world
        .remove_doodad_by_id(id)
        .ok_or(DoodadAuthoringError::DoodadNotFound(id))
}

/// Borrow a doodad record by id.
pub fn lookup_doodad(world: &WorldData, id: DoodadId) -> Option<&DoodadRecord> {
    world.get_doodad(id)
}

fn validate_scale(min: f32, max: f32, scale: Vec3) -> Result<(), DoodadAuthoringError> {
    for component in [scale.x, scale.y, scale.z] {
        if component < min || component > max {
            return Err(DoodadAuthoringError::ScaleOutOfRange { min, max, scale });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, DoodadCatalog, DoodadMetadata, LocalPosition};

    fn layout_world() -> WorldData {
        WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn catalog() -> DoodadCatalog {
        DoodadCatalog::default()
    }

    fn position(chunk_x: i32, chunk_z: i32, local: Vec3) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(chunk_x, chunk_z),
            LocalPosition::new(local),
        )
    }

    #[test]
    fn create_doodad_from_definition() {
        let cat = catalog();
        let mut world = layout_world();
        let def = DoodadDefinitionId::new("tree_oak");
        let pos = position(1, 2, Vec3::new(64.0, 0.0, 128.0));

        let record = create_doodad(
            &cat,
            &mut world,
            &def,
            pos,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        assert_eq!(record.definition_id, def);
        assert_eq!(record.placement.position, pos);
        assert_eq!(lookup_doodad(&world, record.id).unwrap().id, record.id);
        world.assert_doodad_index_consistent();
    }

    #[test]
    fn definition_lookup_failure() {
        let cat = catalog();
        let mut world = layout_world();
        let missing = DoodadDefinitionId::new("missing_tree");

        let err = create_doodad(
            &cat,
            &mut world,
            &missing,
            position(0, 0, Vec3::ZERO),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap_err();

        assert_eq!(err, DoodadAuthoringError::DefinitionNotFound(missing));
    }

    #[test]
    fn scale_validation() {
        let cat = catalog();
        let mut world = layout_world();
        let def = DoodadDefinitionId::new("tree_oak");

        let err = create_doodad(
            &cat,
            &mut world,
            &def,
            position(0, 0, Vec3::ZERO),
            DoodadSource::Authored,
            DoodadPlacementOverrides {
                scale: Some(Vec3::splat(2.0)),
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DoodadAuthoringError::ScaleOutOfRange { .. }
        ));
    }

    #[test]
    fn move_within_same_chunk() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_doodad(
            &cat,
            &mut world,
            &DoodadDefinitionId::new("rock_small"),
            position(0, 0, Vec3::new(10.0, 0.0, 20.0)),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        let new_pos = position(0, 0, Vec3::new(200.0, 5.0, 50.0));
        let moved = move_doodad(&mut world, record.id, new_pos).unwrap();

        assert_eq!(moved.id, record.id);
        assert_eq!(moved.placement.position, new_pos);
        assert_eq!(world.doodad_chunk(record.id), Some(crate::world::ChunkId::new(ChunkCoord::new(0, 0))));
    }

    #[test]
    fn move_across_chunk_boundary() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_doodad(
            &cat,
            &mut world,
            &DoodadDefinitionId::new("bush_scrub"),
            position(0, 0, Vec3::new(200.0, 0.0, 200.0)),
            DoodadSource::Procedural { seed: 99 },
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        let new_pos = position(1, 0, Vec3::new(64.0, 0.0, 64.0));
        let moved = move_doodad(&mut world, record.id, new_pos).unwrap();

        assert_eq!(moved.placement.position, new_pos);
        assert_eq!(
            world.doodad_chunk(record.id),
            Some(crate::world::ChunkId::new(ChunkCoord::new(1, 0)))
        );
        assert!(world
            .doodads_in_chunk(crate::world::ChunkId::new(ChunkCoord::new(0, 0)))
            .is_none());
    }

    #[test]
    fn remove_doodad_by_authoring_id() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_doodad(
            &cat,
            &mut world,
            &DoodadDefinitionId::new("ruin_stone"),
            position(2, 3, Vec3::new(128.0, 0.0, 128.0)),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        let removed = remove_doodad(&mut world, record.id).unwrap();
        assert_eq!(removed.id, record.id);
        assert!(lookup_doodad(&world, record.id).is_none());
        world.assert_doodad_index_consistent();
    }

    #[test]
    fn lookup_doodad_returns_record() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_doodad(
            &cat,
            &mut world,
            &DoodadDefinitionId::new("resource_node_iron"),
            position(0, 1, Vec3::new(1.0, 0.0, 2.0)),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        assert_eq!(
            lookup_doodad(&world, record.id).unwrap().definition_id,
            DoodadDefinitionId::new("resource_node_iron")
        );
    }

    #[test]
    fn id_preserved_after_move() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_doodad(
            &cat,
            &mut world,
            &DoodadDefinitionId::new("tree_dead"),
            position(0, 0, Vec3::new(1.0, 0.0, 1.0)),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        move_doodad(&mut world, record.id, position(3, 4, Vec3::new(50.0, 0.0, 50.0))).unwrap();
        assert_eq!(lookup_doodad(&world, record.id).unwrap().id, record.id);
        world.assert_doodad_index_consistent();
    }

    #[test]
    fn definition_id_preserved_after_move() {
        let cat = catalog();
        let mut world = layout_world();
        let def = DoodadDefinitionId::new("rock_large");
        let record = create_doodad(
            &cat,
            &mut world,
            &def,
            position(0, 0, Vec3::new(1.0, 0.0, 1.0)),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        move_doodad(&mut world, record.id, position(1, 1, Vec3::new(1.0, 0.0, 1.0))).unwrap();
        assert_eq!(lookup_doodad(&world, record.id).unwrap().definition_id, def);
    }

    #[test]
    fn metadata_preserved_after_move() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_doodad(
            &cat,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            position(0, 0, Vec3::new(1.0, 0.0, 1.0)),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
        assert_eq!(lookup_doodad(&world, record.id).unwrap().metadata, DoodadMetadata);

        move_doodad(&mut world, record.id, position(1, 0, Vec3::new(1.0, 0.0, 1.0))).unwrap();
        assert_eq!(lookup_doodad(&world, record.id).unwrap().metadata, DoodadMetadata);
    }

    #[test]
    fn source_preserved_after_move() {
        let cat = catalog();
        let mut world = layout_world();
        let source = DoodadSource::Procedural { seed: 1234 };
        let record = create_doodad(
            &cat,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            position(0, 0, Vec3::new(1.0, 0.0, 1.0)),
            source,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        move_doodad(&mut world, record.id, position(2, 0, Vec3::new(1.0, 0.0, 1.0))).unwrap();
        assert_eq!(lookup_doodad(&world, record.id).unwrap().source, source);
    }
}
