//! Central building navigation movement authority (IN-11g / IN-11gD).
//!
//! Runtime unit movement has two building-specific states:
//! - **Blueprint-controlled** — hydrated runtime navigation geometry is sole building authority.
//! - **Ghost** — no building-specific movement obstruction (no legacy footprint fallback).
//!
//! Blueprint *resolution* (authored/generated data exists) without runtime hydration is still
//! Ghost for movement until geometry is hydrated (cold-load parity is a separate slice).

use crate::world::{BuildingId, WorldData};

/// Who owns movement/path-blocking for one building instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingNavigationMovementAuthority {
    /// Hydrated runtime navigation blueprint controls boundaries, interiors, and entrances.
    BlueprintControlled(BuildingId),
    /// No hydrated runtime blueprint geometry — building does not block unit movement.
    Ghost,
}

/// Resolve movement authority for one building.
///
/// Active [`BuildingNavigationRuntime`] is the signal that runtime navigation geometry was
/// hydrated — not merely that blueprint data resolves in catalog or editor.
pub fn building_navigation_movement_authority(
    world: &WorldData,
    building_id: BuildingId,
) -> BuildingNavigationMovementAuthority {
    if world
        .building_navigation_runtime()
        .get(building_id)
        .is_some()
    {
        BuildingNavigationMovementAuthority::BlueprintControlled(building_id)
    } else {
        BuildingNavigationMovementAuthority::Ghost
    }
}

pub fn building_uses_blueprint_movement_authority(
    world: &WorldData,
    building_id: BuildingId,
) -> bool {
    matches!(
        building_navigation_movement_authority(world, building_id),
        BuildingNavigationMovementAuthority::BlueprintControlled(_)
    )
}

/// Label for overlay / diagnostics.
pub fn movement_authority_label(world: &WorldData, building_id: BuildingId) -> &'static str {
    match building_navigation_movement_authority(world, building_id) {
        BuildingNavigationMovementAuthority::BlueprintControlled(_) => "Blueprint",
        BuildingNavigationMovementAuthority::Ghost => "Ghost",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::fixtures::one_region_doorless_navigation_blueprint;
    use crate::world::{
        BuildingDefinitionId, BuildingLifecycleState, BuildingOwnership, BuildingSource,
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId,
        DoodadPlacementOverrides, DoodadSource, FootprintCatalog, InteriorProfileCatalog,
        LocalPosition, NavigationConfig, OccupancyCatalogs, PassabilityAgent,
        PassabilityBlockReason, PassabilityCatalogs, PassabilityResult, WorldPosition,
        create_building, create_doodad, find_path, place_player_building, query_passability_at,
        query_static_occupancy_at, set_building_lifecycle_stage,
    };
    use bevy::prelude::*;

    fn layout_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
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

    fn passability_catalogs<'a>(
        doodad: &'a DoodadCatalog,
        building: &'a crate::world::BuildingCatalog,
        footprint: &'a FootprintCatalog,
    ) -> PassabilityCatalogs<'a> {
        PassabilityCatalogs {
            doodad,
            building,
            footprint,
        }
    }

    #[test]
    fn blueprint_building_skips_legacy_footprint_blocking() {
        let mut world = layout_world();
        let building_catalog = crate::world::BuildingCatalog::default();
        let nav_catalog = crate::world::BuildingNavigationBlueprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let occupancy = OccupancyCatalogs {
            building: &building_catalog,
            doodad: &doodad,
            footprint: &footprint,
        };
        let interior = InteriorProfileCatalog::default();
        let blueprint = one_region_doorless_navigation_blueprint();
        let placement = pos(50.0, 50.0);
        let id = place_player_building(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            placement,
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
            occupancy,
        )
        .unwrap()
        .id;
        world
            .mutate_building(id, |record| {
                record.interior.navigation_blueprint_override = Some(
                    crate::world::BuildingNavigationBlueprintInstanceOverride::inline(blueprint),
                );
            })
            .unwrap();
        set_building_lifecycle_stage(
            &mut world,
            &building_catalog,
            &interior,
            &doodad,
            occupancy,
            Some(&nav_catalog),
            id,
            BuildingLifecycleState::Complete,
            1.0,
        )
        .unwrap();
        assert!(matches!(
            building_navigation_movement_authority(&world, id),
            BuildingNavigationMovementAuthority::BlueprintControlled(_)
        ));
        let center = world.get_building(id).unwrap().placement.position;
        let result = query_static_occupancy_at(&world, occupancy, center, 0.5);
        assert!(
            !result.blocked,
            "blueprint building footprint must not block"
        );
    }

    #[test]
    fn no_runtime_blueprint_building_is_navigation_ghost() {
        let world = layout_world();
        let building_catalog = crate::world::BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let occupancy = OccupancyCatalogs {
            building: &building_catalog,
            doodad: &doodad,
            footprint: &footprint,
        };
        let mut world = world;
        create_building(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            pos(50.0, 50.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            None,
        )
        .unwrap();
        let id = world.sorted_building_ids()[0];
        assert_eq!(
            building_navigation_movement_authority(&world, id),
            BuildingNavigationMovementAuthority::Ghost
        );
        let center = world.get_building(id).unwrap().placement.position;
        assert!(
            !query_static_occupancy_at(&world, occupancy, center, 0.5).blocked,
            "ghost building footprint must not block static occupancy for movement"
        );
    }

    #[test]
    fn ghost_building_find_path_through_footprint_doodad_still_blocks() {
        let mut world = layout_world();
        let building_catalog = crate::world::BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let catalogs = passability_catalogs(&doodad, &building_catalog, &footprint);
        let nav_config = NavigationConfig::default();

        create_building(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            pos(50.0, 50.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            None,
        )
        .unwrap();

        let start = pos(40.0, 50.0);
        let through = pos(50.0, 50.0);
        let goal = pos(60.0, 50.0);

        let path = find_path(&world, catalogs, &nav_config, 0.5, 40.0, start, goal)
            .expect("ghost building must not block surface path through footprint");
        assert!(
            path.waypoints.len() >= 2,
            "expected a routed path across the ghost building footprint"
        );
        assert!(
            matches!(
                query_passability_at(
                    &world,
                    catalogs,
                    through,
                    PassabilityAgent {
                        radius_meters: 0.5,
                        max_slope_degrees: 40.0,
                    },
                ),
                PassabilityResult::Passable { .. }
            ),
            "position at ghost building footprint must be passable"
        );

        create_doodad(
            &doodad,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            through,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();

        assert!(
            matches!(
                query_passability_at(
                    &world,
                    catalogs,
                    through,
                    PassabilityAgent {
                        radius_meters: 0.5,
                        max_slope_degrees: 40.0,
                    },
                ),
                PassabilityResult::Blocked {
                    reason: PassabilityBlockReason::DoodadOccupied,
                    ..
                }
            ),
            "doodad on ghost-building footprint must still block movement"
        );
    }
}
