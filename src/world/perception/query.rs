//! Bounded unit perception queries (ADR-132 Phase 4).

use crate::world::unit::{UnitId, UnitRecord};
use crate::world::{UnitCatalog, WorldData};

/// Default authored Sight Range matching legacy combat AI scan radius (24 m).
pub const DEFAULT_SIGHT_RANGE_METERS: f32 = 24.0;

/// Resolve the observer's authored Sight Range from its unit definition.
pub fn sight_range_meters_for_record(unit_catalog: &UnitCatalog, record: &UnitRecord) -> f32 {
    unit_catalog
        .get(&record.definition_id)
        .map(|definition| definition.sight_range_meters)
        .unwrap_or(DEFAULT_SIGHT_RANGE_METERS)
}

/// Units the observer can currently perceive — within authored Sight Range only.
///
/// Uses chunk-local [`WorldData::query_units_in_radius`]; the observer is excluded.
/// Results are sorted by [`UnitId`] (deterministic).
pub fn perceived_units(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    observer_id: UnitId,
) -> Vec<UnitId> {
    let Some(observer) = world.get_unit(observer_id) else {
        return Vec::new();
    };
    let sight_range = sight_range_meters_for_record(unit_catalog, observer);
    let candidates =
        world.query_units_in_radius(observer.placement.position, sight_range, Some(observer_id));
    #[cfg(feature = "dev")]
    super::trace::perception_query(observer_id, sight_range, &candidates);
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        UnitCatalog, UnitDefinition, UnitDefinitionId, UnitOwnership, UnitRenderKey, UnitSource,
        WeaponDefinitionId, WorldPosition, create_unit_with_ownership,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
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

    fn pos_chunk(cx: i32, cz: i32, x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(cx, cz),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn catalog_with_sight(sight_range_meters: f32) -> UnitCatalog {
        UnitCatalog::from_definitions(vec![
            UnitDefinition::new_test(
                UnitDefinitionId::new("scout"),
                "Scout",
                "Wild",
                1,
                5,
                5,
                4,
                4,
                4,
                4,
                4,
                4,
                10.0,
                "Common",
                4.0,
                0.5,
                30.0,
                WeaponDefinitionId::new("weapon_fists"),
                true,
                UnitRenderKey::reserved("wolf"),
            )
            .with_sight_range_meters(sight_range_meters),
        ])
        .unwrap()
    }

    #[test]
    fn perception_returns_units_inside_sight_range() {
        let catalog = catalog_with_sight(10.0);
        let mut world = flat_world();
        let observer = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id;
        let near = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(8.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let perceived = perceived_units(&world, &catalog, observer);
        assert_eq!(perceived, vec![near]);
    }

    #[test]
    fn perception_excludes_units_outside_sight_range() {
        let catalog = catalog_with_sight(10.0);
        let mut world = flat_world();
        let observer = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id;
        create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(15.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        assert!(perceived_units(&world, &catalog, observer).is_empty());
    }

    #[test]
    fn perception_excludes_observer() {
        let catalog = catalog_with_sight(10.0);
        let mut world = flat_world();
        let observer = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id;
        assert!(perceived_units(&world, &catalog, observer).is_empty());
    }

    #[test]
    fn perception_order_is_deterministic() {
        let catalog = catalog_with_sight(20.0);
        let mut world = flat_world();
        let observer = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id;
        let b = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(5.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
        )
        .unwrap()
        .id;
        let a = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(0.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let first = perceived_units(&world, &catalog, observer);
        let second = perceived_units(&world, &catalog, observer);
        assert_eq!(first, second);
        assert_eq!(first, vec![a.min(b), a.max(b)]);
    }

    #[test]
    fn perception_uses_bounded_chunk_query_not_distant_units() {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        for (cx, cz) in [(0, 0), (5, 0)] {
            let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
            world.insert(
                ChunkId::new(ChunkCoord::new(cx, cz)),
                ChunkData::new(heightfield, Vec::new()),
            );
        }
        let catalog = catalog_with_sight(10.0);
        let observer = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos(0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id;
        let distant = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            pos_chunk(5, 0, 0.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        assert!(world.sorted_unit_ids().contains(&distant));
        assert!(perceived_units(&world, &catalog, observer).is_empty());
    }

    #[test]
    fn missing_definition_uses_default_sight_range() {
        use crate::world::ownership::UnitOwnership;
        use crate::world::unit::UnitRecord;
        use crate::world::{UnitDefinitionId, UnitPlacement, UnitSource};

        let catalog = UnitCatalog::default();
        let record = UnitRecord::new(
            crate::world::UnitId::new(1),
            UnitDefinitionId::new("missing_definition"),
            UnitPlacement::new(pos(0.0, 0.0), bevy::prelude::Quat::IDENTITY),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
            5,
            crate::world::FactionId::new("wild"),
            crate::world::SpeciesId::new("wolf"),
        );
        assert_eq!(
            sight_range_meters_for_record(&catalog, &record),
            DEFAULT_SIGHT_RANGE_METERS
        );
    }
}
