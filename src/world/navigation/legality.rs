//! Universal movement legality primitives (IN-11gF).
//!
//! Authoritative point and segment queries consumed by passability adapters, pathfinding,
//! simplification, movement execution, and (later) Blocked Area visualization.

use bevy::prelude::*;

use super::grid::{NavigationAgent, NavigationConfig};
use crate::world::occupancy::{
    OccupancyError, OccupancySource, PassabilityAgent, PassabilityBlockReason, PassabilityCatalogs,
    PassabilityResult, PassabilityUnavailableReason, is_position_blocked_by_static_occupancy,
    query_static_occupancy_at,
};
use crate::world::{
    ChunkLayout, SlopeWalkability, SpaceId, SpaceRegistry, WorldData, WorldPosition,
    building_uses_blueprint_movement_authority, classify_slope_walkability,
    ground_position_in_space, ground_world_position, interior_agent_fits_region,
    interior_segment_respects_region_boundary, surface_blueprint_support_blocks_position,
    surface_segment_respects_blueprint_boundaries,
};

/// Why a movement segment is illegal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavigationSegmentBlockReason {
    BlueprintRegionBoundary,
    RegionBoundary,
    PointBlocked(PassabilityBlockReason),
    TerrainUnavailable,
    GroundingFailed,
}

/// Structured segment legality result.
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationSegmentLegality {
    Legal,
    Blocked {
        reason: NavigationSegmentBlockReason,
        source: Option<OccupancySource>,
    },
}

impl NavigationSegmentLegality {
    pub fn is_legal(&self) -> bool {
        matches!(self, NavigationSegmentLegality::Legal)
    }
}

/// Authoritative point legality: can this agent occupy this position in this space?
pub fn query_navigation_point_legality(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
    space_id: SpaceId,
) -> PassabilityResult {
    if space_id.is_surface() {
        query_surface_point_legality(world, catalogs, position, agent)
    } else {
        query_interior_point_legality(world, catalogs, position, agent, space_id)
    }
}

fn query_surface_point_legality(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
) -> PassabilityResult {
    let Some(grounded) = ground_world_position(world, position) else {
        return PassabilityResult::Unavailable {
            reason: PassabilityUnavailableReason::TerrainUnavailable,
        };
    };

    match classify_slope_walkability(world, grounded, agent.max_slope_degrees) {
        SlopeWalkability::Walkable => {}
        SlopeWalkability::Unavailable => {
            return PassabilityResult::Unavailable {
                reason: PassabilityUnavailableReason::TerrainUnavailable,
            };
        }
        SlopeWalkability::TooSteep => {
            return PassabilityResult::Blocked {
                reason: PassabilityBlockReason::SlopeTooSteep,
                source: None,
            };
        }
    }

    let layout = world.layout();
    let point_xz = grounded.to_global(layout).xz();
    if let Some(building_id) =
        surface_blueprint_support_blocks_position(world, layout, point_xz, agent.radius_meters)
    {
        return PassabilityResult::Blocked {
            reason: PassabilityBlockReason::BlueprintSupport,
            source: Some(OccupancySource::Building(building_id)),
        };
    }

    let occupancy =
        query_static_occupancy_at(world, catalogs.occupancy(), grounded, agent.radius_meters);
    if occupancy.blocked {
        let reason = match occupancy.source {
            Some(OccupancySource::Building(_)) => PassabilityBlockReason::BuildingOccupied,
            Some(OccupancySource::Doodad(_)) => PassabilityBlockReason::DoodadOccupied,
            None => PassabilityBlockReason::InvalidCell,
        };
        return PassabilityResult::Blocked {
            reason,
            source: occupancy.source,
        };
    }
    if let Some(error) = occupancy.error {
        return map_occupancy_error(error);
    }

    PassabilityResult::Passable {
        movement_cost_multiplier: 1.0,
    }
}

fn query_interior_point_legality(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
    space_id: SpaceId,
) -> PassabilityResult {
    let _ = catalogs;
    if !(agent.radius_meters >= 0.0) || !agent.radius_meters.is_finite() {
        return PassabilityResult::Blocked {
            reason: PassabilityBlockReason::InvalidCell,
            source: None,
        };
    }
    let layout = world.layout();
    if !interior_agent_fits_region(
        world.building_navigation_runtime(),
        world.space_registry(),
        layout,
        position,
        space_id,
        agent.radius_meters,
    ) {
        return PassabilityResult::Blocked {
            reason: PassabilityBlockReason::AgentClearanceInsufficient,
            source: None,
        };
    }
    let owning_building = world
        .space_registry()
        .get_space(space_id)
        .and_then(|space| space.owning_building_id);
    if let Some(building_id) = owning_building {
        if building_uses_blueprint_movement_authority(world, building_id) {
            return PassabilityResult::Passable {
                movement_cost_multiplier: 1.0,
            };
        }
    }
    if interior_static_occupancy_blocked(world, catalogs.occupancy(), position, agent.radius_meters)
    {
        return PassabilityResult::Blocked {
            reason: PassabilityBlockReason::BuildingOccupied,
            source: None,
        };
    }
    let center = position.to_global(layout);
    let center_xz = Vec2::new(center.x, center.z);
    let cell = crate::world::occupancy_cell_at_global_xz(center_xz);
    let chunk = crate::world::chunk_for_occupancy_cell(cell, layout);
    let chunk_id = crate::world::ChunkId::new(chunk);
    if let Some(grid) = world.occupancy_in_chunk(chunk_id) {
        if let Some(entry) = grid.get(cell, space_id.raw()) {
            if matches!(entry.state, crate::world::OccupancyState::Blocked) {
                return PassabilityResult::Blocked {
                    reason: PassabilityBlockReason::BuildingOccupied,
                    source: Some(entry.source),
                };
            }
        }
    }
    PassabilityResult::Passable {
        movement_cost_multiplier: 1.0,
    }
}

fn map_occupancy_error(error: OccupancyError) -> PassabilityResult {
    PassabilityResult::Blocked {
        reason: match error {
            OccupancyError::MissingBuildingDefinition(_)
            | OccupancyError::MissingDoodadDefinition { .. }
            | OccupancyError::MissingFootprint(_) => PassabilityBlockReason::MissingDefinition,
            OccupancyError::InvalidRotation { .. }
            | OccupancyError::InvalidMaskDimensions { .. }
            | OccupancyError::MeshDerivedRequiresFootprintId
            | OccupancyError::DisabledFootprint(_)
            | OccupancyError::CollisionNodeMissing { .. }
            | OccupancyError::BakeFailed(_)
            | OccupancyError::NonFiniteGeometry
            | OccupancyError::OverrideOutOfBounds { .. }
            | OccupancyError::OverrideConflict { .. } => PassabilityBlockReason::CorruptFootprint,
            OccupancyError::OccupancyConflict { .. }
            | OccupancyError::RegistrationIndexMismatch => PassabilityBlockReason::InvalidCell,
            OccupancyError::InvalidBlockingRadius { .. } => PassabilityBlockReason::InvalidCell,
        },
        source: None,
    }
}

fn interior_static_occupancy_blocked(
    world: &WorldData,
    catalogs: crate::world::occupancy::OccupancyCatalogs<'_>,
    position: WorldPosition,
    agent_radius_meters: f32,
) -> bool {
    is_position_blocked_by_static_occupancy(world, catalogs, position, agent_radius_meters)
}

/// Authoritative segment legality: can this agent move continuously from A to B in this space?
pub fn query_navigation_segment_legality(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> NavigationSegmentLegality {
    if space_id.is_surface() {
        if !surface_segment_respects_blueprint_boundaries(
            world,
            from,
            to,
            layout,
            agent.radius_meters,
        ) {
            return NavigationSegmentLegality::Blocked {
                reason: NavigationSegmentBlockReason::BlueprintRegionBoundary,
                source: None,
            };
        }
        return surface_segment_sampling_legality(world, catalogs, config, agent, from, to, layout);
    }

    if !interior_segment_respects_region_boundary(
        world.building_navigation_runtime(),
        space_registry,
        layout,
        from,
        to,
        space_id,
        agent.radius_meters,
    ) {
        return NavigationSegmentLegality::Blocked {
            reason: NavigationSegmentBlockReason::RegionBoundary,
            source: None,
        };
    }
    interior_segment_sampling_legality(
        world,
        space_registry,
        catalogs,
        config,
        space_id,
        agent,
        from,
        to,
        layout,
    )
}

fn surface_segment_sampling_legality(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> NavigationSegmentLegality {
    let surface_config = config.config_for_space(SpaceId::SURFACE);
    let from_global = from.to_global(layout);
    let to_global = to.to_global(layout);
    let delta = Vec2::new(to_global.x - from_global.x, to_global.z - from_global.z);
    let distance = delta.length();
    if distance <= 1e-4 {
        return NavigationSegmentLegality::Legal;
    }

    let sample_spacing = surface_config.cell_spacing_meters * 0.5;
    let steps = ((distance / sample_spacing).ceil() as usize).max(1);
    let passability_agent = PassabilityAgent::from(agent);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let global = from_global.lerp(to_global, t);
        let candidate = WorldPosition::from_global(Vec3::new(global.x, 0.0, global.z), layout);
        let Some(grounded) = ground_world_position(world, candidate) else {
            return NavigationSegmentLegality::Blocked {
                reason: NavigationSegmentBlockReason::TerrainUnavailable,
                source: None,
            };
        };
        let point = query_navigation_point_legality(
            world,
            catalogs,
            grounded,
            passability_agent,
            SpaceId::SURFACE,
        );
        if !matches!(point, PassabilityResult::Passable { .. }) {
            return point_blocked_legality(point);
        }
    }
    NavigationSegmentLegality::Legal
}

fn interior_segment_sampling_legality(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    space_id: SpaceId,
    agent: NavigationAgent,
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> NavigationSegmentLegality {
    let space_config = config.config_for_space(space_id);
    let from_global = from.to_global(layout);
    let to_global = to.to_global(layout);
    let delta = Vec2::new(to_global.x - from_global.x, to_global.z - from_global.z);
    let distance = delta.length();
    let passability_agent = PassabilityAgent::from(agent);
    if distance <= 1e-4 {
        let point =
            query_navigation_point_legality(world, catalogs, from, passability_agent, space_id);
        return if matches!(point, PassabilityResult::Passable { .. }) {
            NavigationSegmentLegality::Legal
        } else {
            point_blocked_legality(point)
        };
    }

    let sample_spacing = space_config.cell_spacing_meters * 0.5;
    let steps = ((distance / sample_spacing).ceil() as usize).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let global = from_global.lerp(to_global, t);
        let candidate = WorldPosition::from_global(Vec3::new(global.x, 0.0, global.z), layout);
        let Some(grounded) = ground_position_in_space(world, space_registry, space_id, candidate)
        else {
            return NavigationSegmentLegality::Blocked {
                reason: NavigationSegmentBlockReason::GroundingFailed,
                source: None,
            };
        };
        let point =
            query_navigation_point_legality(world, catalogs, grounded, passability_agent, space_id);
        if !matches!(point, PassabilityResult::Passable { .. }) {
            return point_blocked_legality(point);
        }
    }
    NavigationSegmentLegality::Legal
}

fn point_blocked_legality(point: PassabilityResult) -> NavigationSegmentLegality {
    match point {
        PassabilityResult::Passable { .. } => NavigationSegmentLegality::Blocked {
            reason: NavigationSegmentBlockReason::PointBlocked(PassabilityBlockReason::InvalidCell),
            source: None,
        },
        PassabilityResult::Blocked { reason, source } => NavigationSegmentLegality::Blocked {
            reason: NavigationSegmentBlockReason::PointBlocked(reason),
            source,
        },
        PassabilityResult::Unavailable { .. } => NavigationSegmentLegality::Blocked {
            reason: NavigationSegmentBlockReason::TerrainUnavailable,
            source: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState,
        BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
        BuildingNavigationBlueprintInstanceOverride, BuildingOwnership, BuildingSource, ChunkCoord,
        ChunkData, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId,
        DoodadPlacementOverrides, DoodadSource, FootprintCatalog, Heightfield,
        InteriorProfileCatalog, LocalPosition, NavigationConfig, NavigationEntranceDefinition,
        NavigationFloorDefinition, NavigationPolygon2d, NavigationRegionDefinition,
        OccupancyCatalogs, create_building, create_doodad, is_position_walkable_in_space,
        navigation_segment_valid, place_player_building, query_passability_at,
        query_passability_in_space, set_building_lifecycle_stage,
    };
    use bevy::prelude::{Quat, Vec3};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
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

    fn pass<'a>(
        doodad: &'a DoodadCatalog,
        building: &'a BuildingCatalog,
        footprint: &'a FootprintCatalog,
    ) -> PassabilityCatalogs<'a> {
        PassabilityCatalogs {
            doodad,
            building,
            footprint,
        }
    }

    fn agent(radius: f32) -> PassabilityAgent {
        PassabilityAgent {
            radius_meters: radius,
            max_slope_degrees: 40.0,
        }
    }

    fn nav_agent(radius: f32) -> NavigationAgent {
        NavigationAgent {
            radius_meters: radius,
            max_slope_degrees: 40.0,
        }
    }

    fn activate_fixture(
        world: &mut WorldData,
        blueprint: BuildingNavigationBlueprint,
        placement: WorldPosition,
    ) -> crate::world::BuildingId {
        let building_catalog = BuildingCatalog::default();
        let nav_catalog = BuildingNavigationBlueprintCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let occupancy = OccupancyCatalogs {
            building: &building_catalog,
            doodad: &doodad_catalog,
            footprint: &footprint,
        };
        let interior = InteriorProfileCatalog::default();
        let id = place_player_building(
            &building_catalog,
            world,
            &BuildingDefinitionId::new("hut"),
            placement,
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            occupancy,
        )
        .unwrap()
        .id;
        world
            .mutate_building(id, |record| {
                record.interior.navigation_blueprint_override = Some(
                    BuildingNavigationBlueprintInstanceOverride::inline(blueprint),
                );
            })
            .expect("building");
        set_building_lifecycle_stage(
            world,
            &building_catalog,
            &interior,
            &doodad_catalog,
            occupancy,
            Some(&nav_catalog),
            id,
            BuildingLifecycleState::Complete,
            1.0,
        )
        .unwrap();
        id
    }

    fn local_xz_to_world(
        world: &WorldData,
        building_id: crate::world::BuildingId,
        local_xz: Vec2,
        floor_y: f32,
    ) -> WorldPosition {
        let runtime = world
            .building_navigation_runtime()
            .get(building_id)
            .expect("runtime");
        let layout = world.layout();
        let global = runtime
            .model_transform
            .transform_point(Vec3::new(local_xz.x, floor_y, local_xz.y));
        WorldPosition::from_global(global, layout)
    }

    fn region_space(
        world: &WorldData,
        building_id: crate::world::BuildingId,
        floor_key: &str,
        region_key: &str,
    ) -> SpaceId {
        let runtime = world
            .building_navigation_runtime()
            .get(building_id)
            .expect("runtime");
        let key = format!("{floor_key}/{region_key}");
        *runtime.space_keys.get(&key).unwrap_or_else(|| {
            panic!("missing space key `{key}`");
        })
    }

    fn one_region_test_blueprint() -> BuildingNavigationBlueprint {
        BuildingNavigationBlueprint::new("one_region_hut", "One Region Hut")
            .with_floors(vec![NavigationFloorDefinition {
                floor_id: 0,
                key: "ground".to_string(),
                display_label: "Ground".to_string(),
                elevation_meters: 0.0,
                visibility_group_id: 1,
                room_tag: None,
                walkable_outline_legacy: None,
                regions: vec![NavigationRegionDefinition {
                    key: "main".to_string(),
                    display_label: "Main".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d {
                        vertices_xz: vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]],
                    },
                }],
            }])
            .with_entrances(vec![NavigationEntranceDefinition {
                key: "exterior_entrance".to_string(),
                floor_key: "ground".to_string(),
                region_key: Some("main".to_string()),
                local_position_xz: [4.0, 0.0],
                radius_meters: 1.5,
                interior_spawn_local: [4.0, 0.0, 1.5],
                bidirectional: true,
                door_key: None,
            }])
    }

    fn oversized_concave_hut_blueprint() -> BuildingNavigationBlueprint {
        BuildingNavigationBlueprint::new("oversized_concave_hut", "Oversized Concave Hut")
            .with_floors(vec![NavigationFloorDefinition {
                floor_id: 0,
                key: "ground".to_string(),
                display_label: "Ground".to_string(),
                elevation_meters: 1.27,
                visibility_group_id: 1,
                room_tag: None,
                walkable_outline_legacy: None,
                regions: vec![NavigationRegionDefinition {
                    key: "main".to_string(),
                    display_label: "Main".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d {
                        vertices_xz: vec![
                            [0.0, 0.0],
                            [14.0, 0.0],
                            [14.0, 14.0],
                            [6.0, 14.0],
                            [6.0, 6.0],
                            [0.0, 6.0],
                        ],
                    },
                }],
            }])
            .with_entrances(vec![NavigationEntranceDefinition {
                key: "exterior_entrance".to_string(),
                floor_key: "ground".to_string(),
                region_key: Some("main".to_string()),
                local_position_xz: [7.0, 0.0],
                radius_meters: 1.5,
                interior_spawn_local: [7.0, 1.27, 1.5],
                bidirectional: true,
                door_key: None,
            }])
    }

    #[test]
    fn open_surface_point_is_legal() {
        let (doodad, building, footprint) = (
            DoodadCatalog::default(),
            BuildingCatalog::default(),
            FootprintCatalog::default(),
        );
        let world = flat_world();
        let result = query_navigation_point_legality(
            &world,
            pass(&doodad, &building, &footprint),
            pos(100.0, 100.0),
            agent(0.5),
            SpaceId::SURFACE,
        );
        assert!(matches!(result, PassabilityResult::Passable { .. }));
    }

    #[test]
    fn ghost_building_footprint_does_not_block_point() {
        let building_catalog = BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let mut world = flat_world();
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
        let center = world
            .get_building(world.sorted_building_ids()[0])
            .unwrap()
            .placement
            .position;
        let result = query_navigation_point_legality(
            &world,
            pass(&doodad, &building_catalog, &footprint),
            center,
            agent(0.5),
            SpaceId::SURFACE,
        );
        assert!(matches!(result, PassabilityResult::Passable { .. }));
    }

    #[test]
    fn doodad_blocks_point_legality() {
        let doodad = DoodadCatalog::default();
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let mut world = flat_world();
        create_doodad(
            &doodad,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(50.0, 50.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();
        let result = query_navigation_point_legality(
            &world,
            pass(&doodad, &building, &footprint),
            pos(50.0, 50.0),
            agent(0.5),
            SpaceId::SURFACE,
        );
        assert!(matches!(
            result,
            PassabilityResult::Blocked {
                reason: PassabilityBlockReason::DoodadOccupied,
                ..
            }
        ));
    }

    #[test]
    fn interior_point_inside_region_is_legal() {
        let mut world = flat_world();
        let building_id =
            activate_fixture(&mut world, one_region_test_blueprint(), pos(80.0, 80.0));
        let interior_space = region_space(&world, building_id, "ground", "main");
        let interior_point = local_xz_to_world(&world, building_id, Vec2::new(4.0, 3.0), 0.0);
        let building_catalog = BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let result = query_navigation_point_legality(
            &world,
            pass(&doodad, &building_catalog, &footprint),
            interior_point,
            agent(0.5),
            interior_space,
        );
        assert!(matches!(result, PassabilityResult::Passable { .. }));
    }

    #[test]
    fn interior_point_outside_region_is_blocked() {
        let mut world = flat_world();
        let building_id =
            activate_fixture(&mut world, one_region_test_blueprint(), pos(80.0, 80.0));
        let interior_space = region_space(&world, building_id, "ground", "main");
        let outside = local_xz_to_world(&world, building_id, Vec2::new(-2.0, 3.0), 0.0);
        let building_catalog = BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let result = query_navigation_point_legality(
            &world,
            pass(&doodad, &building_catalog, &footprint),
            outside,
            agent(0.5),
            interior_space,
        );
        assert!(matches!(
            result,
            PassabilityResult::Blocked {
                reason: PassabilityBlockReason::AgentClearanceInsufficient,
                ..
            }
        ));
    }

    #[test]
    fn agent_radius_affects_interior_point_legality() {
        let mut world = flat_world();
        let building_id =
            activate_fixture(&mut world, one_region_test_blueprint(), pos(80.0, 80.0));
        let interior_space = region_space(&world, building_id, "ground", "main");
        let floor_y = world
            .space_registry()
            .get_space(interior_space)
            .map(|space| space.floor_y_global)
            .unwrap_or(0.0);
        let center_line = local_xz_to_world(&world, building_id, Vec2::new(1.2, 3.0), floor_y);
        let near_edge = local_xz_to_world(&world, building_id, Vec2::new(0.35, 3.0), floor_y);
        let building_catalog = BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let catalogs = pass(&doodad, &building_catalog, &footprint);
        assert!(matches!(
            query_navigation_point_legality(
                &world,
                catalogs,
                center_line,
                agent(0.3),
                interior_space
            ),
            PassabilityResult::Passable { .. }
        ));
        assert!(matches!(
            query_navigation_point_legality(
                &world,
                catalogs,
                near_edge,
                agent(1.0),
                interior_space
            ),
            PassabilityResult::Blocked {
                reason: PassabilityBlockReason::AgentClearanceInsufficient,
                ..
            }
        ));
    }

    #[test]
    fn open_surface_segment_is_legal() {
        let (doodad, building, footprint) = (
            DoodadCatalog::default(),
            BuildingCatalog::default(),
            FootprintCatalog::default(),
        );
        let world = flat_world();
        let result = query_navigation_segment_legality(
            &world,
            world.space_registry(),
            pass(&doodad, &building, &footprint),
            NavigationConfig::default(),
            SpaceId::SURFACE,
            nav_agent(0.5),
            pos(40.0, 40.0),
            pos(60.0, 60.0),
            layout(),
        );
        assert!(result.is_legal());
    }

    #[test]
    fn interior_segment_inside_region_is_legal() {
        let mut world = flat_world();
        let building_id =
            activate_fixture(&mut world, one_region_test_blueprint(), pos(80.0, 80.0));
        let interior_space = region_space(&world, building_id, "ground", "main");
        let from = local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), 0.0);
        let to = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0), 0.0);
        let building_catalog = BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let result = query_navigation_segment_legality(
            &world,
            world.space_registry(),
            pass(&doodad, &building_catalog, &footprint),
            NavigationConfig::default(),
            interior_space,
            nav_agent(0.5),
            from,
            to,
            layout(),
        );
        assert!(result.is_legal());
    }

    #[test]
    fn interior_segment_crossing_boundary_is_illegal() {
        let mut world = flat_world();
        let building_id = activate_fixture(
            &mut world,
            oversized_concave_hut_blueprint(),
            pos(80.0, 80.0),
        );
        let interior_space = region_space(&world, building_id, "ground", "main");
        let from = local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), 1.27);
        let to = local_xz_to_world(&world, building_id, Vec2::new(9.0, 9.0), 1.27);
        let building_catalog = BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let result = query_navigation_segment_legality(
            &world,
            world.space_registry(),
            pass(&doodad, &building_catalog, &footprint),
            NavigationConfig::default(),
            interior_space,
            nav_agent(0.6),
            from,
            to,
            layout(),
        );
        assert!(matches!(
            result,
            NavigationSegmentLegality::Blocked {
                reason: NavigationSegmentBlockReason::RegionBoundary,
                ..
            }
        ));
    }

    #[test]
    fn passability_adapter_matches_universal_point_legality() {
        let (doodad, building, footprint) = (
            DoodadCatalog::default(),
            BuildingCatalog::default(),
            FootprintCatalog::default(),
        );
        let world = flat_world();
        let catalogs = pass(&doodad, &building, &footprint);
        let position = pos(120.0, 120.0);
        let passability_agent = agent(0.5);
        assert_eq!(
            query_passability_at(&world, catalogs, position, passability_agent),
            query_navigation_point_legality(
                &world,
                catalogs,
                position,
                passability_agent,
                SpaceId::SURFACE,
            )
        );
        assert_eq!(
            query_passability_in_space(
                &world,
                catalogs,
                position,
                passability_agent,
                SpaceId::SURFACE,
            ),
            query_navigation_point_legality(
                &world,
                catalogs,
                position,
                passability_agent,
                SpaceId::SURFACE,
            )
        );
    }

    #[test]
    fn segment_adapter_matches_universal_segment_legality() {
        let (doodad, building, footprint) = (
            DoodadCatalog::default(),
            BuildingCatalog::default(),
            FootprintCatalog::default(),
        );
        let world = flat_world();
        let catalogs = pass(&doodad, &building, &footprint);
        let config = NavigationConfig::default();
        let nav = nav_agent(0.5);
        let from = pos(30.0, 30.0);
        let to = pos(50.0, 50.0);
        let universal = query_navigation_segment_legality(
            &world,
            world.space_registry(),
            catalogs,
            config,
            SpaceId::SURFACE,
            nav,
            from,
            to,
            layout(),
        );
        let legacy = navigation_segment_valid(
            &world,
            world.space_registry(),
            catalogs,
            config,
            SpaceId::SURFACE,
            nav,
            from,
            to,
            layout(),
        );
        assert_eq!(universal.is_legal(), legacy);
    }

    #[test]
    fn position_walkable_adapter_matches_universal_point_legality() {
        let mut world = flat_world();
        let building_id =
            activate_fixture(&mut world, one_region_test_blueprint(), pos(80.0, 80.0));
        let interior_space = region_space(&world, building_id, "ground", "main");
        let interior_point = local_xz_to_world(&world, building_id, Vec2::new(4.0, 3.0), 0.0);
        let building_catalog = BuildingCatalog::default();
        let doodad = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let catalogs = pass(&doodad, &building_catalog, &footprint);
        let nav = nav_agent(0.5);
        let universal = matches!(
            query_navigation_point_legality(
                &world,
                catalogs,
                interior_point,
                agent(0.5),
                interior_space,
            ),
            PassabilityResult::Passable { .. }
        );
        let adapter = is_position_walkable_in_space(
            &world,
            world.space_registry(),
            catalogs,
            interior_point,
            nav,
            interior_space,
        );
        assert_eq!(universal, adapter);
    }
}
