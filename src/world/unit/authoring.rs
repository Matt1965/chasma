//! Authoritative unit placement API (ADR-027 U2, ADR-051 O1).
//!
//! Operates on [`crate::world::WorldData`] and [`super::catalog::UnitCatalog`].
//! No ECS entities, rendering, movement validation, or save/load.

use bevy::prelude::*;

use super::catalog::UnitCatalog;
use super::id::UnitId;
use super::placement::UnitPlacement;
use super::record::UnitRecord;
use super::source::UnitSource;
use crate::world::ownership::{default_ownership_for_source, UnitOwnership};
use crate::world::{UnitDefinitionId, UnitInsertError, WorldData, WorldPosition};

/// Why an authoring operation failed (ADR-027 U2).
#[derive(Debug, Clone, PartialEq)]
pub enum UnitAuthoringError {
    DefinitionNotFound(UnitDefinitionId),
    DefinitionDisabled(UnitDefinitionId),
    UnitNotFound(UnitId),
    ChunkPlacementMismatch,
}

/// Create a unit with explicit runtime ownership.
pub fn create_unit_with_ownership(
    catalog: &UnitCatalog,
    world: &mut WorldData,
    definition_id: &UnitDefinitionId,
    position: WorldPosition,
    source: UnitSource,
    ownership: UnitOwnership,
) -> Result<UnitRecord, UnitAuthoringError> {
    let definition = catalog
        .get(definition_id)
        .ok_or_else(|| UnitAuthoringError::DefinitionNotFound(definition_id.clone()))?;

    if !definition.enabled {
        return Err(UnitAuthoringError::DefinitionDisabled(
            definition_id.clone(),
        ));
    }

    let id = world.allocate_unit_id();
    let record = UnitRecord::new(
        id,
        definition.id.clone(),
        UnitPlacement::new(position, Quat::IDENTITY),
        source,
        ownership,
        definition.max_hp,
    );

    let chunk = crate::world::ChunkId::new(position.chunk);
    world
        .insert_unit(chunk, record.clone())
        .map_err(|error| match error {
            UnitInsertError::ChunkPlacementMismatch => UnitAuthoringError::ChunkPlacementMismatch,
            UnitInsertError::UnitNotFound => UnitAuthoringError::UnitNotFound(id),
        })?;

    Ok(record)
}

/// Create a unit instance using safe default ownership for [`UnitSource`].
///
/// Does **not** derive ownership from catalog `faction_tag`.
pub fn create_unit(
    catalog: &UnitCatalog,
    world: &mut WorldData,
    definition_id: &UnitDefinitionId,
    position: WorldPosition,
    source: UnitSource,
) -> Result<UnitRecord, UnitAuthoringError> {
    create_unit_with_ownership(
        catalog,
        world,
        definition_id,
        position,
        source,
        default_ownership_for_source(source),
    )
}

/// Move an existing unit to a new world position, including cross-chunk moves.
pub fn move_unit(
    world: &mut WorldData,
    id: UnitId,
    new_position: WorldPosition,
) -> Result<UnitRecord, UnitAuthoringError> {
    world
        .relocate_unit(id, new_position)
        .map_err(|error| match error {
            UnitInsertError::ChunkPlacementMismatch => UnitAuthoringError::ChunkPlacementMismatch,
            UnitInsertError::UnitNotFound => UnitAuthoringError::UnitNotFound(id),
        })
}

/// Remove a unit by id, returning the removed record.
pub fn remove_unit(world: &mut WorldData, id: UnitId) -> Result<UnitRecord, UnitAuthoringError> {
    world
        .remove_unit_by_id(id)
        .ok_or(UnitAuthoringError::UnitNotFound(id))
}

/// Borrow a unit record by id.
pub fn lookup_unit(world: &WorldData, id: UnitId) -> Option<&UnitRecord> {
    world.get_unit(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ownership::{Affiliation, UnitOwnership};
    use crate::world::{ChunkCoord, LocalPosition, UnitCatalog};

    fn layout_world() -> WorldData {
        WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn catalog() -> UnitCatalog {
        UnitCatalog::default()
    }

    fn position(chunk_x: i32, chunk_z: i32, local: Vec3) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(chunk_x, chunk_z),
            LocalPosition::new(local),
        )
    }

    #[test]
    fn create_unit_starts_at_full_hp() {
        let cat = catalog();
        let mut world = layout_world();
        let def_id = UnitDefinitionId::new("wolf");
        let expected_max = cat.get(&def_id).unwrap().max_hp;

        let record = create_unit(
            &cat,
            &mut world,
            &def_id,
            position(0, 0, Vec3::ZERO),
            UnitSource::Authored,
        )
        .unwrap();

        assert_eq!(record.vitals.current_hp, expected_max);
        assert_eq!(record.vitals.max_hp, expected_max);
        assert_eq!(record.combat_state, crate::world::CombatState::Peaceful);
    }

    #[test]
    fn create_unit_from_definition() {
        let cat = catalog();
        let mut world = layout_world();
        let def = UnitDefinitionId::new("wolf");
        let pos = position(1, 2, Vec3::new(64.0, 0.0, 128.0));

        let record = create_unit(&cat, &mut world, &def, pos, UnitSource::Authored).unwrap();

        assert_eq!(record.definition_id, def);
        assert_eq!(record.placement.position, pos);
        assert_eq!(lookup_unit(&world, record.id).unwrap().id, record.id);
        world.assert_unit_index_consistent();
    }

    #[test]
    fn create_unit_with_ownership_stores_fields() {
        let cat = catalog();
        let mut world = layout_world();
        let ownership = UnitOwnership::player_default();
        let record = create_unit_with_ownership(
            &cat,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            position(0, 0, Vec3::ZERO),
            UnitSource::Authored,
            ownership,
        )
        .unwrap();
        assert_eq!(record.owner_id, ownership.owner_id);
        assert_eq!(record.team_id, ownership.team_id);
        assert_eq!(record.affiliation, Affiliation::Player);
    }

    #[test]
    fn disabled_definition_rejected() {
        let mut cat = UnitCatalog::default();
        let mut def = cat.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
        def.enabled = false;
        cat = UnitCatalog::from_definitions(vec![def]).unwrap();

        let mut world = layout_world();
        let err = create_unit(
            &cat,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            position(0, 0, Vec3::ZERO),
            UnitSource::Authored,
        )
        .unwrap_err();

        assert_eq!(
            err,
            UnitAuthoringError::DefinitionDisabled(UnitDefinitionId::new("wolf"))
        );
    }
}
