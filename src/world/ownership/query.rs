//! Ownership query helpers — read-only scans over [`WorldData`] (ADR-051 O1).

use crate::world::{UnitId, UnitRecord, WorldData};

use super::defaults::DEFAULT_PLAYER_OWNER_ID;
use super::types::{Affiliation, OwnerId, UnitOwnership};

/// Whether this record is commandable by the local human player.
pub fn is_player_controllable(record: &UnitRecord) -> bool {
    record.affiliation == Affiliation::Player && record.owner_id == Some(DEFAULT_PLAYER_OWNER_ID)
}

/// Whether `record` is owned by `owner`.
pub fn is_owned_by(record: &UnitRecord, owner: OwnerId) -> bool {
    record.owner_id == Some(owner)
}

/// All player-controllable unit ids, sorted deterministically.
pub fn player_units(world: &WorldData) -> Vec<UnitId> {
    units_by_affiliation(world, Affiliation::Player)
        .into_iter()
        .filter(|id| world.get_unit(*id).is_some_and(is_player_controllable))
        .collect()
}

/// All units with the given owner id, sorted deterministically.
#[cfg(test)]
pub(crate) fn units_by_owner(world: &WorldData, owner: OwnerId) -> Vec<UnitId> {
    world
        .sorted_unit_ids()
        .into_iter()
        .filter(|id| world.get_unit(*id).is_some_and(|r| is_owned_by(r, owner)))
        .collect()
}

/// All units with the given affiliation, sorted deterministically.
pub fn units_by_affiliation(world: &WorldData, affiliation: Affiliation) -> Vec<UnitId> {
    world
        .sorted_unit_ids()
        .into_iter()
        .filter(|id| {
            world
                .get_unit(*id)
                .is_some_and(|r| r.affiliation == affiliation)
        })
        .collect()
}

/// Safe default ownership when callers use legacy [`create_unit`] without explicit ownership.
pub fn default_ownership_for_source(source: crate::world::UnitSource) -> UnitOwnership {
    use crate::world::UnitSource;

    match source {
        UnitSource::Authored => UnitOwnership::neutral(),
        UnitSource::Dev => UnitOwnership::dev_local_player(),
        UnitSource::Procedural { .. } => UnitOwnership::wildlife(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitCatalog,
        UnitDefinitionId, UnitSource, WorldPosition, create_unit, create_unit_with_ownership,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn player_owned_units_are_controllable() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let record = world.get_unit(id).unwrap();
        assert!(is_player_controllable(record));
    }

    #[test]
    fn neutral_units_are_not_controllable() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        assert!(!is_player_controllable(world.get_unit(id).unwrap()));
    }

    #[test]
    fn hostile_units_are_not_controllable() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id;
        assert!(!is_player_controllable(world.get_unit(id).unwrap()));
    }

    #[test]
    fn units_by_owner_returns_matching_set() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let a = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let _neutral = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(2.0, 2.0),
            UnitSource::Authored,
        )
        .unwrap();

        let owned = units_by_owner(&world, DEFAULT_PLAYER_OWNER_ID);
        assert_eq!(owned, vec![a]);
    }

    #[test]
    fn units_by_affiliation_is_deterministic() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let a = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let b = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(2.0, 2.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id;

        assert_eq!(units_by_affiliation(&world, Affiliation::Player), vec![a]);
        assert_eq!(units_by_affiliation(&world, Affiliation::Hostile), vec![b]);
        assert_eq!(player_units(&world), vec![a]);
    }

    #[test]
    fn default_create_unit_uses_neutral_for_authored() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let record = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(0.0, 0.0),
            UnitSource::Authored,
        )
        .unwrap();
        assert_eq!(record.affiliation, Affiliation::Neutral);
        assert!(record.owner_id.is_none());
    }

    #[test]
    fn ownership_survives_relocate_and_grounding() {
        use crate::world::{ground_unit_to_terrain, move_unit};

        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let ownership = UnitOwnership::player_default();
        let id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            ownership,
        )
        .unwrap()
        .id;
        let before = world.get_unit(id).unwrap().ownership();
        move_unit(&mut world, id, pos(20.0, 20.0)).unwrap();
        let _ = ground_unit_to_terrain(&mut world, id);
        assert_eq!(world.get_unit(id).unwrap().ownership(), before);
    }
}
