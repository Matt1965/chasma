//! Mutual-perception relationship link discovery for dev overlay (ADR-132 consumer).

use crate::world::relationship::{
    AuthoredRelationshipCatalog, RelationshipStandingStore, effective_relationship_for_records,
};
use crate::world::{
    UnitCatalog, UnitId, WorldData, is_unit_alive, sight_range_meters_for_record, xz_distance,
};

/// Dev-only cap on link draw distance; not perception authority.
///
/// Default authored sight range is 24 m. 32 m covers normal mutually detecting combatants
/// with modest headroom if perception rules expand, without drawing extreme-range pairs.
pub const RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS: f32 = 32.0;

/// One undirected pair with independent directional relationship totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipLinkPair {
    pub unit_a: UnitId,
    pub unit_b: UnitId,
    pub a_to_b: i32,
    pub b_to_a: i32,
}

/// Format a signed relationship total for overlay display (`+150`, `0`, `-50`).
pub fn format_signed_relationship(value: i32) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

/// Discover mutually perceiving unit pairs using bounded spatial queries (not O(N²)).
pub fn discover_mutual_perception_relationship_links(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    authored: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    viz_max_distance_meters: f32,
) -> Vec<RelationshipLinkPair> {
    let mut pairs = Vec::new();

    for observer_id in world.sorted_unit_ids() {
        let Some(observer) = world.get_unit(observer_id) else {
            continue;
        };
        if !is_unit_alive(observer) {
            continue;
        }

        let sight_range = sight_range_meters_for_record(unit_catalog, observer);
        let candidates = world.query_units_in_radius(
            observer.placement.position,
            sight_range,
            Some(observer_id),
        );

        for candidate_id in candidates {
            if observer_id >= candidate_id {
                continue;
            }
            let Some(candidate) = world.get_unit(candidate_id) else {
                continue;
            };
            if !is_unit_alive(candidate) {
                continue;
            }

            let center_distance = xz_distance(
                observer.placement.position,
                candidate.placement.position,
                world.layout(),
            );
            if center_distance > viz_max_distance_meters {
                continue;
            }

            let candidate_sight = sight_range_meters_for_record(unit_catalog, candidate);
            if center_distance > candidate_sight {
                continue;
            }

            pairs.push(RelationshipLinkPair {
                unit_a: observer_id,
                unit_b: candidate_id,
                a_to_b: effective_relationship_for_records(authored, standing, observer, candidate),
                b_to_a: effective_relationship_for_records(authored, standing, candidate, observer),
            });
        }
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::relationship::{
        AuthoredFacetKey, AuthoredRelationshipCatalog, DirectedRelationshipEdgeKey, FactionId,
    };
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
            UnitDefinition::new_test(
                UnitDefinitionId::new("player_scout"),
                "Player Scout",
                "Player",
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
                UnitRenderKey::reserved("robot"),
            )
            .with_sight_range_meters(sight_range_meters),
        ])
        .unwrap()
    }

    fn phase6_authored_catalog() -> AuthoredRelationshipCatalog {
        AuthoredRelationshipCatalog::from_edges([(
            DirectedRelationshipEdgeKey::new(
                AuthoredFacetKey::Faction(FactionId::new("wild")),
                AuthoredFacetKey::Faction(FactionId::new("player")),
            ),
            -300,
        )])
        .expect("edge")
    }

    fn wild_unit(world: &mut WorldData, catalog: &UnitCatalog, id_offset: u64) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("scout"),
            pos(id_offset as f32 * 4.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id
    }

    fn player_unit(world: &mut WorldData, catalog: &UnitCatalog, x: f32) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("player_scout"),
            pos(x, 0.0),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
        )
        .unwrap()
        .id
    }

    #[test]
    fn format_signed_relationship_values() {
        assert_eq!(format_signed_relationship(150), "+150");
        assert_eq!(format_signed_relationship(0), "0");
        assert_eq!(format_signed_relationship(-50), "-50");
    }

    #[test]
    fn one_way_perception_produces_no_link() {
        let catalog = catalog_with_sight(10.0);
        let mut world = flat_world();
        let observer = wild_unit(&mut world, &catalog, 0);
        player_unit(&mut world, &catalog, 15.0);
        let links = discover_mutual_perception_relationship_links(
            &world,
            &catalog,
            &AuthoredRelationshipCatalog::default(),
            world.relationship_standing_store(),
            RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
        );
        assert!(links.is_empty());
        let _ = observer;
    }

    #[test]
    fn mutual_perception_produces_one_link() {
        let catalog = catalog_with_sight(20.0);
        let mut world = flat_world();
        let a = wild_unit(&mut world, &catalog, 0);
        let b = player_unit(&mut world, &catalog, 8.0);
        let links = discover_mutual_perception_relationship_links(
            &world,
            &catalog,
            &phase6_authored_catalog(),
            world.relationship_standing_store(),
            RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
        );
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].unit_a, a.min(b));
        assert_eq!(links[0].unit_b, a.max(b));
    }

    #[test]
    fn pair_is_not_rendered_twice() {
        let catalog = catalog_with_sight(20.0);
        let mut world = flat_world();
        wild_unit(&mut world, &catalog, 0);
        player_unit(&mut world, &catalog, 8.0);
        let links = discover_mutual_perception_relationship_links(
            &world,
            &catalog,
            &AuthoredRelationshipCatalog::default(),
            world.relationship_standing_store(),
            RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
        );
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn directional_totals_are_independently_resolved() {
        let catalog = catalog_with_sight(20.0);
        let mut world = flat_world();
        wild_unit(&mut world, &catalog, 0);
        player_unit(&mut world, &catalog, 8.0);
        let links = discover_mutual_perception_relationship_links(
            &world,
            &catalog,
            &phase6_authored_catalog(),
            world.relationship_standing_store(),
            RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
        );
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].a_to_b, -300);
        assert_eq!(links[0].b_to_a, 0);
    }

    #[test]
    fn asymmetric_values_map_to_correct_direction() {
        let authored = AuthoredRelationshipCatalog::from_edges([
            (
                DirectedRelationshipEdgeKey::new(
                    AuthoredFacetKey::Faction(FactionId::new("wild")),
                    AuthoredFacetKey::Faction(FactionId::new("player")),
                ),
                -80,
            ),
            (
                DirectedRelationshipEdgeKey::new(
                    AuthoredFacetKey::Faction(FactionId::new("player")),
                    AuthoredFacetKey::Faction(FactionId::new("wild")),
                ),
                40,
            ),
        ])
        .expect("edges");
        let catalog = catalog_with_sight(20.0);
        let mut world = flat_world();
        let wild = wild_unit(&mut world, &catalog, 0);
        let player = player_unit(&mut world, &catalog, 8.0);
        let links = discover_mutual_perception_relationship_links(
            &world,
            &catalog,
            &authored,
            world.relationship_standing_store(),
            RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
        );
        assert_eq!(links.len(), 1);
        let link = &links[0];
        assert_eq!(link.unit_a, wild.min(player));
        assert_eq!(link.unit_b, wild.max(player));
        if link.unit_a == wild {
            assert_eq!(link.a_to_b, -80);
            assert_eq!(link.b_to_a, 40);
        } else {
            assert_eq!(link.a_to_b, 40);
            assert_eq!(link.b_to_a, -80);
        }
    }

    #[test]
    fn link_disappears_when_mutual_perception_ends() {
        let catalog = catalog_with_sight(10.0);
        let mut world = flat_world();
        wild_unit(&mut world, &catalog, 0);
        let player = player_unit(&mut world, &catalog, 8.0);
        assert_eq!(
            discover_mutual_perception_relationship_links(
                &world,
                &catalog,
                &AuthoredRelationshipCatalog::default(),
                world.relationship_standing_store(),
                RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
            )
            .len(),
            1
        );
        world
            .update_unit_position(player, pos(25.0, 0.0))
            .expect("move player out of range");
        assert!(
            discover_mutual_perception_relationship_links(
                &world,
                &catalog,
                &AuthoredRelationshipCatalog::default(),
                world.relationship_standing_store(),
                RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
            )
            .is_empty()
        );
    }

    #[test]
    fn visualization_cap_does_not_replace_perception_semantics() {
        let catalog = catalog_with_sight(40.0);
        let mut world = flat_world();
        wild_unit(&mut world, &catalog, 0);
        player_unit(&mut world, &catalog, 30.0);
        let links = discover_mutual_perception_relationship_links(
            &world,
            &catalog,
            &AuthoredRelationshipCatalog::default(),
            world.relationship_standing_store(),
            20.0,
        );
        assert!(links.is_empty());
        let links = discover_mutual_perception_relationship_links(
            &world,
            &catalog,
            &AuthoredRelationshipCatalog::default(),
            world.relationship_standing_store(),
            RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
        );
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn dead_units_do_not_produce_links() {
        use crate::world::UnitState;

        let catalog = catalog_with_sight(20.0);
        let mut world = flat_world();
        let wild = wild_unit(&mut world, &catalog, 0);
        let player = player_unit(&mut world, &catalog, 8.0);
        assert_eq!(
            discover_mutual_perception_relationship_links(
                &world,
                &catalog,
                &AuthoredRelationshipCatalog::default(),
                world.relationship_standing_store(),
                RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
            )
            .len(),
            1
        );
        world.set_unit_state(player, UnitState::Dead).unwrap();
        world.set_unit_hp(player, 0).unwrap();
        assert!(
            discover_mutual_perception_relationship_links(
                &world,
                &catalog,
                &AuthoredRelationshipCatalog::default(),
                world.relationship_standing_store(),
                RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
            )
            .is_empty()
        );
        let _ = wild;
    }

    #[test]
    fn discovery_uses_bounded_queries_not_global_pair_scan() {
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
        wild_unit(&mut world, &catalog, 0);
        create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("scout"),
            WorldPosition::new(
                ChunkCoord::new(5, 0),
                LocalPosition::new(Vec3::new(0.0, 0.0, 0.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap();
        assert_eq!(world.sorted_unit_ids().len(), 2);
        assert!(
            discover_mutual_perception_relationship_links(
                &world,
                &catalog,
                &AuthoredRelationshipCatalog::default(),
                world.relationship_standing_store(),
                RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
            )
            .is_empty()
        );
    }
}
