//! Dev-only Surface goal passability diagnostics (IN-11gI-E-T2).
//!
//! Read-only probes mirroring `query_surface_point_legality` stages without
//! mutating navigation behavior.

#[cfg(feature = "dev")]
use bevy::prelude::*;

#[cfg(feature = "dev")]
use crate::world::{
    ChunkId, ChunkLayout, NavigationError, OccupancySource, PassabilityAgent,
    PassabilityBlockReason, PassabilityCatalogs, PassabilityResult, PassabilityUnavailableReason,
    SlopeWalkability, SpaceId, UnitOwnership, WorldData, WorldPosition, classify_slope_walkability,
    ground_world_position, position_in_surface_entrance_portal, query_navigation_point_legality,
    query_static_occupancy_at, slope_at, space_route_for_unit, try_sample_height_at_position,
};

#[cfg(feature = "dev")]
#[derive(Debug, Clone)]
pub struct SurfacePointLegalityProbe {
    pub position: WorldPosition,
    pub global: Vec3,
    pub chunk_label: String,
    pub terrain_chunk_loaded: bool,
    pub terrain_sample_available: bool,
    pub terrain_ground_y: Option<f32>,
    pub neighbor_height_px: Option<f32>,
    pub neighbor_height_nx: Option<f32>,
    pub neighbor_height_pz: Option<f32>,
    pub neighbor_height_nz: Option<f32>,
    pub slope_degrees: Option<f32>,
    pub max_slope_degrees: f32,
    pub slope_walkability: Option<String>,
    pub portal_exemption: bool,
    pub static_occupancy_blocked: bool,
    pub static_occupancy_label: String,
    pub blocking_object_kind: Option<String>,
    pub blocking_object_id: Option<u64>,
    pub building_overlap_blocks: bool,
    pub authority_regression: bool,
    pub point_legality: String,
    pub passability_block_reason: Option<String>,
    pub passability_unavailable_reason: Option<String>,
    pub first_rejecting_subcheck: String,
}

#[cfg(feature = "dev")]
#[derive(Debug, Clone)]
pub struct SurfaceLocalSample {
    pub offset_label: String,
    pub point_legality: String,
    pub passability_block_reason: Option<String>,
    pub passability_unavailable_reason: Option<String>,
}

#[cfg(feature = "dev")]
#[derive(Debug, Clone)]
pub struct SurfaceExitPassabilityProbe {
    pub goal: SurfacePointLegalityProbe,
    pub staging: Option<SurfacePointLegalityProbe>,
    pub local_samples: Vec<SurfaceLocalSample>,
}

#[cfg(feature = "dev")]
pub fn probe_interior_to_surface_exit_passability(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    agent_radius_meters: f32,
    max_slope_degrees: f32,
    grounded_goal: WorldPosition,
    start_space: SpaceId,
    goal_space: SpaceId,
    unit_ownership: Option<UnitOwnership>,
) -> SurfaceExitPassabilityProbe {
    let agent = PassabilityAgent {
        radius_meters: agent_radius_meters,
        max_slope_degrees,
    };
    let goal = probe_surface_point(world, catalogs, grounded_goal, agent);
    let staging = resolve_surface_staging_position(world, start_space, goal_space, unit_ownership)
        .map(|position| probe_surface_point(world, catalogs, position, agent));
    let local_samples = probe_local_surface_samples(world, catalogs, grounded_goal, agent);
    SurfaceExitPassabilityProbe {
        goal,
        staging,
        local_samples,
    }
}

#[cfg(feature = "dev")]
fn resolve_surface_staging_position(
    world: &WorldData,
    start_space: SpaceId,
    goal_space: SpaceId,
    unit_ownership: Option<UnitOwnership>,
) -> Option<WorldPosition> {
    let registry = world.space_registry();
    let layout = world.layout();
    let route = space_route_for_unit(world, start_space, goal_space, unit_ownership)
        .or_else(|| registry.space_route(start_space, goal_space))?;
    let portal_id = route.first().copied()?;
    let portal = registry.get_portal(portal_id)?;
    let (dest_space, staging) =
        portal.destination_for_planning(start_space, layout, world, registry)?;
    if !dest_space.is_surface() {
        return None;
    }
    Some(staging)
}

#[cfg(feature = "dev")]
fn probe_local_surface_samples(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    center: WorldPosition,
    agent: PassabilityAgent,
) -> Vec<SurfaceLocalSample> {
    let layout = world.layout();
    let center_global = center.to_global(layout);
    let offsets: [(f32, f32, &str); 9] = [
        (0.0, 0.0, "(0,0)"),
        (1.0, 0.0, "(+1,0)"),
        (-1.0, 0.0, "(-1,0)"),
        (0.0, 1.0, "(0,+1)"),
        (0.0, -1.0, "(0,-1)"),
        (1.0, 1.0, "(+1,+1)"),
        (1.0, -1.0, "(+1,-1)"),
        (-1.0, 1.0, "(-1,+1)"),
        (-1.0, -1.0, "(-1,-1)"),
    ];
    offsets
        .into_iter()
        .map(|(dx, dz, label)| {
            let global = Vec3::new(center_global.x + dx, 0.0, center_global.z + dz);
            let position = WorldPosition::from_global(global, layout);
            let legality =
                query_navigation_point_legality(world, catalogs, position, agent, SpaceId::SURFACE);
            SurfaceLocalSample {
                offset_label: label.to_string(),
                point_legality: legality_label(&legality),
                passability_block_reason: block_reason_label(&legality),
                passability_unavailable_reason: unavailable_reason_label(&legality),
            }
        })
        .collect()
}

#[cfg(feature = "dev")]
fn probe_surface_point(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: PassabilityAgent,
) -> SurfacePointLegalityProbe {
    let layout = world.layout();
    let global = position.to_global(layout);
    let chunk_id = ChunkId::new(position.chunk);
    let terrain_chunk_loaded = world.get(chunk_id).is_some();
    let terrain_sample_available = try_sample_height_at_position(world, position).is_ok();
    let neighbor_height_px = sample_neighbor_height(world, layout, global, 1.0, 0.0);
    let neighbor_height_nx = sample_neighbor_height(world, layout, global, -1.0, 0.0);
    let neighbor_height_pz = sample_neighbor_height(world, layout, global, 0.0, 1.0);
    let neighbor_height_nz = sample_neighbor_height(world, layout, global, 0.0, -1.0);

    let mut first_rejecting_subcheck = "NONE".to_string();
    let mut terrain_ground_y = None;
    let mut slope_degrees = None;
    let mut slope_walkability = None;
    let mut portal_exemption = false;
    let mut static_occupancy_blocked = false;
    let mut static_occupancy_label = "not_queried".to_string();
    let mut blocking_object_kind = None;
    let mut blocking_object_id = None;
    let mut building_overlap_blocks = false;

    let grounded = match ground_world_position(world, position) {
        Some(grounded) => {
            terrain_ground_y = Some(grounded.to_global(layout).y);
            grounded
        }
        None => {
            first_rejecting_subcheck = "TERRAIN_GROUND_UNAVAILABLE".to_string();
            let legality =
                query_navigation_point_legality(world, catalogs, position, agent, SpaceId::SURFACE);
            return finish_probe(
                position,
                global,
                chunk_label(position.chunk),
                terrain_chunk_loaded,
                terrain_sample_available,
                terrain_ground_y,
                neighbor_height_px,
                neighbor_height_nx,
                neighbor_height_pz,
                neighbor_height_nz,
                slope_degrees,
                agent.max_slope_degrees,
                slope_walkability,
                portal_exemption,
                static_occupancy_blocked,
                static_occupancy_label,
                blocking_object_kind,
                blocking_object_id,
                building_overlap_blocks,
                legality,
                first_rejecting_subcheck,
            );
        }
    };

    match classify_slope_walkability(world, grounded, agent.max_slope_degrees) {
        SlopeWalkability::Walkable => {
            slope_walkability = Some("Walkable".to_string());
            slope_degrees = slope_at(world, grounded).ok();
        }
        SlopeWalkability::Unavailable => {
            slope_walkability = Some("Unavailable".to_string());
            slope_degrees = slope_at(world, grounded).ok();
            if first_rejecting_subcheck == "NONE" {
                first_rejecting_subcheck = "SLOPE_UNAVAILABLE".to_string();
            }
        }
        SlopeWalkability::TooSteep => {
            slope_walkability = Some("TooSteep".to_string());
            slope_degrees = slope_at(world, grounded).ok();
            if first_rejecting_subcheck == "NONE" {
                first_rejecting_subcheck = "SLOPE_TOO_STEEP".to_string();
            }
        }
    }

    portal_exemption =
        position_in_surface_entrance_portal(world.space_registry(), layout, grounded);
    if portal_exemption && first_rejecting_subcheck == "NONE" {
        let legality = PassabilityResult::Passable {
            movement_cost_multiplier: 1.0,
        };
        return finish_probe(
            position,
            global,
            chunk_label(position.chunk),
            terrain_chunk_loaded,
            terrain_sample_available,
            terrain_ground_y,
            neighbor_height_px,
            neighbor_height_nx,
            neighbor_height_pz,
            neighbor_height_nz,
            slope_degrees,
            agent.max_slope_degrees,
            slope_walkability,
            portal_exemption,
            static_occupancy_blocked,
            static_occupancy_label,
            blocking_object_kind,
            blocking_object_id,
            building_overlap_blocks,
            legality,
            first_rejecting_subcheck,
        );
    }

    if first_rejecting_subcheck != "NONE" {
        let legality =
            query_navigation_point_legality(world, catalogs, position, agent, SpaceId::SURFACE);
        return finish_probe(
            position,
            global,
            chunk_label(position.chunk),
            terrain_chunk_loaded,
            terrain_sample_available,
            terrain_ground_y,
            neighbor_height_px,
            neighbor_height_nx,
            neighbor_height_pz,
            neighbor_height_nz,
            slope_degrees,
            agent.max_slope_degrees,
            slope_walkability,
            portal_exemption,
            static_occupancy_blocked,
            static_occupancy_label,
            blocking_object_kind,
            blocking_object_id,
            building_overlap_blocks,
            legality,
            first_rejecting_subcheck,
        );
    }

    let occupancy =
        query_static_occupancy_at(world, catalogs.occupancy(), grounded, agent.radius_meters);
    static_occupancy_blocked = occupancy.blocked;
    static_occupancy_label = if occupancy.blocked {
        "blocked".to_string()
    } else if occupancy.error.is_some() {
        "error".to_string()
    } else {
        "clear".to_string()
    };
    if let Some(source) = occupancy.source {
        match source {
            OccupancySource::Building(id) => {
                blocking_object_kind = Some("building".to_string());
                blocking_object_id = Some(id.raw());
                building_overlap_blocks = occupancy.blocked;
            }
            OccupancySource::Doodad(id) => {
                blocking_object_kind = Some("doodad".to_string());
                blocking_object_id = Some(id.raw());
            }
        }
    }
    if occupancy.blocked && first_rejecting_subcheck == "NONE" {
        first_rejecting_subcheck = match occupancy.source {
            Some(OccupancySource::Building(_)) => "STATIC_OCCUPANCY_BUILDING".to_string(),
            Some(OccupancySource::Doodad(_)) => "STATIC_OCCUPANCY_DOODAD".to_string(),
            None => "STATIC_OCCUPANCY_UNKNOWN".to_string(),
        };
    } else if occupancy.error.is_some() && first_rejecting_subcheck == "NONE" {
        first_rejecting_subcheck = "STATIC_OCCUPANCY_ERROR".to_string();
    }

    let legality =
        query_navigation_point_legality(world, catalogs, position, agent, SpaceId::SURFACE);
    finish_probe(
        position,
        global,
        chunk_label(position.chunk),
        terrain_chunk_loaded,
        terrain_sample_available,
        terrain_ground_y,
        neighbor_height_px,
        neighbor_height_nx,
        neighbor_height_pz,
        neighbor_height_nz,
        slope_degrees,
        agent.max_slope_degrees,
        slope_walkability,
        portal_exemption,
        static_occupancy_blocked,
        static_occupancy_label,
        blocking_object_kind,
        blocking_object_id,
        building_overlap_blocks,
        legality,
        first_rejecting_subcheck,
    )
}

#[cfg(feature = "dev")]
fn finish_probe(
    position: WorldPosition,
    global: Vec3,
    chunk_label: String,
    terrain_chunk_loaded: bool,
    terrain_sample_available: bool,
    terrain_ground_y: Option<f32>,
    neighbor_height_px: Option<f32>,
    neighbor_height_nx: Option<f32>,
    neighbor_height_pz: Option<f32>,
    neighbor_height_nz: Option<f32>,
    slope_degrees: Option<f32>,
    max_slope_degrees: f32,
    slope_walkability: Option<String>,
    portal_exemption: bool,
    static_occupancy_blocked: bool,
    static_occupancy_label: String,
    blocking_object_kind: Option<String>,
    blocking_object_id: Option<u64>,
    building_overlap_blocks: bool,
    legality: PassabilityResult,
    first_rejecting_subcheck: String,
) -> SurfacePointLegalityProbe {
    let authority_regression = building_overlap_blocks;
    SurfacePointLegalityProbe {
        position,
        global,
        chunk_label,
        terrain_chunk_loaded,
        terrain_sample_available,
        terrain_ground_y,
        neighbor_height_px,
        neighbor_height_nx,
        neighbor_height_pz,
        neighbor_height_nz,
        slope_degrees,
        max_slope_degrees,
        slope_walkability,
        portal_exemption,
        static_occupancy_blocked,
        static_occupancy_label,
        blocking_object_kind,
        blocking_object_id,
        building_overlap_blocks,
        authority_regression,
        point_legality: legality_label(&legality),
        passability_block_reason: block_reason_label(&legality),
        passability_unavailable_reason: unavailable_reason_label(&legality),
        first_rejecting_subcheck,
    }
}

#[cfg(feature = "dev")]
fn sample_neighbor_height(
    world: &WorldData,
    layout: ChunkLayout,
    center: Vec3,
    dx: f32,
    dz: f32,
) -> Option<f32> {
    let neighbor = WorldPosition::from_global(Vec3::new(center.x + dx, 0.0, center.z + dz), layout);
    try_sample_height_at_position(world, neighbor).ok()
}

#[cfg(feature = "dev")]
fn chunk_label(chunk: crate::world::ChunkCoord) -> String {
    format!("({}, {})", chunk.x, chunk.z)
}

#[cfg(feature = "dev")]
fn legality_label(result: &PassabilityResult) -> String {
    match result {
        PassabilityResult::Passable { .. } => "Passable".to_string(),
        PassabilityResult::Blocked { .. } => "Blocked".to_string(),
        PassabilityResult::Unavailable { .. } => "Unavailable".to_string(),
    }
}

#[cfg(feature = "dev")]
fn block_reason_label(result: &PassabilityResult) -> Option<String> {
    match result {
        PassabilityResult::Blocked { reason, .. } => Some(format_block_reason(*reason)),
        _ => None,
    }
}

#[cfg(feature = "dev")]
fn unavailable_reason_label(result: &PassabilityResult) -> Option<String> {
    match result {
        PassabilityResult::Unavailable { reason } => Some(format_unavailable_reason(*reason)),
        _ => None,
    }
}

#[cfg(feature = "dev")]
fn format_block_reason(reason: PassabilityBlockReason) -> String {
    match reason {
        PassabilityBlockReason::SlopeTooSteep => "SlopeTooSteep".to_string(),
        PassabilityBlockReason::BuildingOccupied => "BuildingOccupied".to_string(),
        PassabilityBlockReason::DoodadOccupied => "DoodadOccupied".to_string(),
        PassabilityBlockReason::CorruptFootprint => "CorruptFootprint".to_string(),
        PassabilityBlockReason::MissingDefinition => "MissingDefinition".to_string(),
        PassabilityBlockReason::InvalidCell => "InvalidCell".to_string(),
        PassabilityBlockReason::AgentClearanceInsufficient => {
            "AgentClearanceInsufficient".to_string()
        }
        PassabilityBlockReason::BlueprintSupport => "BlueprintSupport".to_string(),
    }
}

#[cfg(feature = "dev")]
fn format_unavailable_reason(reason: PassabilityUnavailableReason) -> String {
    match reason {
        PassabilityUnavailableReason::TerrainUnavailable => "TerrainUnavailable".to_string(),
    }
}

#[cfg(feature = "dev")]
pub fn format_surface_point_probe_lines(
    prefix: &str,
    probe: &SurfacePointLegalityProbe,
) -> Vec<String> {
    let global = format!(
        "({:.2},{:.2},{:.2})",
        probe.global.x, probe.global.y, probe.global.z
    );
    let mut lines = vec![
        format!("{prefix}={global}"),
        format!("{prefix}_point_legality={}", probe.point_legality),
        format!(
            "passability_block_reason={}",
            probe.passability_block_reason.as_deref().unwrap_or("none")
        ),
        format!(
            "passability_unavailable_reason={}",
            probe
                .passability_unavailable_reason
                .as_deref()
                .unwrap_or("none")
        ),
        format!(
            "{prefix}_first_rejecting_subcheck={}",
            probe.first_rejecting_subcheck
        ),
        format!("surface_terrain_chunk={}", probe.chunk_label),
        format!("surface_terrain_available={}", probe.terrain_chunk_loaded),
        format!(
            "surface_terrain_sample_available={}",
            probe.terrain_sample_available
        ),
        format!(
            "surface_terrain_ground_y={}",
            probe
                .terrain_ground_y
                .map(|y| format!("{y:.3}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "surface_neighbor_height_px={}",
            probe
                .neighbor_height_px
                .map(|y| format!("{y:.3}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "surface_neighbor_height_nx={}",
            probe
                .neighbor_height_nx
                .map(|y| format!("{y:.3}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "surface_neighbor_height_pz={}",
            probe
                .neighbor_height_pz
                .map(|y| format!("{y:.3}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "surface_neighbor_height_nz={}",
            probe
                .neighbor_height_nz
                .map(|y| format!("{y:.3}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "surface_slope_degrees={}",
            probe
                .slope_degrees
                .map(|s| format!("{s:.3}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("surface_max_slope_degrees={:.3}", probe.max_slope_degrees),
        format!(
            "surface_slope_walkability={}",
            probe.slope_walkability.as_deref().unwrap_or("none")
        ),
        format!("surface_portal_exemption={}", probe.portal_exemption),
        format!("surface_static_occupancy={}", probe.static_occupancy_label),
        format!(
            "surface_blocking_object_kind={}",
            probe.blocking_object_kind.as_deref().unwrap_or("none")
        ),
        format!(
            "surface_blocking_object_id={}",
            probe
                .blocking_object_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "building_overlap_blocks_surface_goal={}",
            probe.building_overlap_blocks
        ),
    ];
    if probe.authority_regression {
        lines.push("AUTHORITY_REGRESSION=true".to_string());
    }
    lines
}

#[cfg(feature = "dev")]
pub fn format_local_sample_line(sample: &SurfaceLocalSample) -> String {
    format!(
        "surface_local_sample offset={} legality={} block_reason={} unavailable_reason={}",
        sample.offset_label,
        sample.point_legality,
        sample.passability_block_reason.as_deref().unwrap_or("none"),
        sample
            .passability_unavailable_reason
            .as_deref()
            .unwrap_or("none")
    )
}

#[cfg(feature = "dev")]
pub fn should_probe_surface_goal_passability(
    path_result: Result<&crate::world::NavigationPath, NavigationError>,
    start_space: SpaceId,
    goal_space: SpaceId,
) -> bool {
    matches!(path_result, Err(NavigationError::GoalBlocked))
        && !start_space.is_surface()
        && goal_space.is_surface()
}

#[cfg(all(test, feature = "dev"))]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId,
        DoodadPlacementOverrides, DoodadSource, FootprintCatalog, Heightfield, LocalPosition,
        WorldData, WorldPosition, create_doodad,
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

    #[test]
    fn surface_probe_reports_doodad_block_reason() {
        let mut world = flat_world();
        let doodad_catalog = DoodadCatalog::default();
        create_doodad(
            &doodad_catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(40.0, 40.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();
        let catalogs = PassabilityCatalogs {
            doodad: &doodad_catalog,
            building: &crate::world::BuildingCatalog::default(),
            footprint: &FootprintCatalog::default(),
        };
        let probe = probe_surface_point(
            &world,
            catalogs,
            pos(40.0, 40.0),
            PassabilityAgent {
                radius_meters: 0.5,
                max_slope_degrees: 45.0,
            },
        );
        assert_eq!(probe.point_legality, "Blocked");
        assert_eq!(
            probe.passability_block_reason.as_deref(),
            Some("DoodadOccupied")
        );
        assert_eq!(probe.first_rejecting_subcheck, "STATIC_OCCUPANCY_DOODAD");
        assert_eq!(probe.blocking_object_kind.as_deref(), Some("doodad"));
    }
}
