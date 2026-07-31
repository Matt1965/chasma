//! Navigation debug overlay — walkability, occupancy, portals, footprints (NV0).
//!
//! Observes navigation and occupancy state only; does not mutate simulation.

use bevy::prelude::*;

use crate::camera::RtsCameraState;
use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::debug::settings::DebugOverlaySettings;
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::primary_selected_unit;
use crate::units::input::SelectedUnits;
use crate::world::{
    BuildingCatalog, ChunkExtent, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog, GridCoord,
    NavigationAgent, NavigationConfig, OccupancyState, PassabilityAgent, PassabilityBlockReason,
    PassabilityCatalogs, PassabilityResult, PortalType, SpaceId, WorldConfig, WorldData,
    effective_building_footprint_for_placement, grid_cell_center_global, grid_coord_at_global_xz,
    is_cell_walkable, occupied_cells_for_footprint_yaw, query_passability_at,
};

use super::helpers::{render_position, xz_to_render_y};
use super::nav_cells::draw_xz_quad;

/// Specialized overlays (footprints / portals / occupancy) keep a local focus radius.
const SPECIALIZED_OVERLAY_RADIUS_METERS: f32 = 96.0;
const MAX_OCCUPANCY_CELLS_DRAWN: u32 = 2_500;

/// Default agent used when sampling walkability for the pathing mask.
pub const DEBUG_NAV_AGENT: NavigationAgent = NavigationAgent {
    radius_meters: 0.5,
    max_slope_degrees: 45.0,
};

/// Rebuild cached mask samples periodically while the toggle stays on.
const MASK_CACHE_REFRESH_FRAMES: u32 = 30;

/// One sampled navigation-mask cell (testable; presentation-only).
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationMaskCellSample {
    pub coord: GridCoord,
    pub center_xz: Vec2,
    pub walkable: bool,
    pub block_reason: Option<PassabilityBlockReason>,
}

/// Compact last-frame draw stats for the pathing mask (not serialized).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NavigationMaskDrawStats {
    pub cells_sampled: u32,
    pub navigable_drawn: u32,
    pub blocked_drawn: u32,
    pub ran: bool,
}

/// Cached whole-world mask samples so passability is not re-queried every frame.
#[derive(Resource, Debug, Clone, Default)]
pub struct NavigationMaskCache {
    pub samples: Vec<NavigationMaskCellSample>,
    pub resident_chunk_count: usize,
    pub frames_since_rebuild: u32,
    pub active: bool,
    /// Bumped whenever samples are rebuilt (mesh sync watches this).
    pub revision: u64,
    pub cell_spacing_meters: f32,
}

pub fn draw_navigation_debug_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    nav_config: Res<NavigationConfig>,
    building_catalog: Res<BuildingCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    settings: Res<DebugOverlaySettings>,
    selection: Res<SelectedUnits>,
    world_selection: Res<WorldSelectionState>,
    camera: Query<&RtsCameraState, With<crate::camera::RtsCamera>>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut mask_stats: ResMut<NavigationMaskDrawStats>,
    mut mask_cache: ResMut<NavigationMaskCache>,
) {
    *mask_stats = NavigationMaskDrawStats::default();
    if !settings.navigation_overlay_active() {
        if mask_cache.active || !mask_cache.samples.is_empty() {
            mask_cache.active = false;
            mask_cache.samples.clear();
            mask_cache.revision = mask_cache.revision.saturating_add(1);
        }
        return;
    }
    mask_stats.ran = true;

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint_catalog,
    };
    // Specialized overlays may still focus near selection/camera; the pathing mask
    // covers the whole loaded/authored world and does not require a selection.
    let focus = debug_focus_global(&world, &selection, &world_selection, &camera, layout);

    if settings.grid || settings.nav_blockers {
        // Sample/cache only — filled mesh presentation is owned by
        // `sync_navigation_mask_meshes` (gizmo lines are not visible from RTS height).
        let stats =
            refresh_navigation_pathing_mask_cache(&world, &catalogs, *nav_config, &mut mask_cache);
        mask_stats.cells_sampled = stats.cells_sampled;
        mask_stats.ran = stats.ran;
        // Drawn counts filled by mesh sync after rebuild.
    } else {
        mask_cache.active = false;
        mask_cache.samples.clear();
        mask_cache.revision = mask_cache.revision.saturating_add(1);
    }

    if settings.nav_occupancy || settings.nav_reservations {
        draw_occupancy_cells(
            &mut gizmos,
            &world,
            layout,
            vertical_scale,
            focus,
            settings.nav_occupancy,
            settings.nav_reservations,
        );
    }

    if settings.nav_footprints {
        draw_building_footprints(
            &mut gizmos,
            &world,
            layout,
            vertical_scale,
            &building_catalog,
            &footprint_catalog,
            focus,
            world_selection.building_id,
        );
    }

    if settings.nav_entrances {
        draw_portal_markers(
            &mut gizmos,
            &world,
            layout,
            vertical_scale,
            focus,
            world_selection.building_id,
        );
    }

    if settings.nav_blueprint || settings.nav_entrances || settings.nav_footprints {
        let active_space = world_selection
            .primary_unit(&selection)
            .or_else(|| primary_selected_unit(&selection))
            .and_then(|unit_id| world.get_unit(unit_id))
            .map(|unit| unit.current_space_id);
        draw_runtime_blueprint_floors(
            &mut gizmos,
            &world,
            layout,
            vertical_scale,
            focus,
            active_space,
            world_selection.building_id,
        );
    }
}

fn debug_focus_global(
    world: &WorldData,
    selection: &SelectedUnits,
    world_selection: &WorldSelectionState,
    camera: &Query<&RtsCameraState, With<crate::camera::RtsCamera>>,
    layout: ChunkLayout,
) -> Vec3 {
    if let Some(unit_id) = world_selection
        .primary_unit(selection)
        .or_else(|| primary_selected_unit(selection))
    {
        if let Some(unit) = world.get_unit(unit_id) {
            return unit.placement.position.to_global(layout);
        }
    }
    if let Some(building_id) = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten()
    {
        if let Some(building) = world.get_building(building_id) {
            return building.placement.position.to_global(layout);
        }
    }
    camera
        .iter()
        .next()
        .map(|state| state.focus)
        .unwrap_or(Vec3::ZERO)
}

/// Inclusive nav-grid bounds covering currently resident (loaded) chunks.
pub fn navigation_mask_world_grid_bounds(
    world: &WorldData,
    nav_config: NavigationConfig,
) -> Option<(GridCoord, GridCoord)> {
    let extent = world.resident_extent()?;
    Some(extent_to_nav_grid_bounds(
        extent,
        world.layout(),
        nav_config,
    ))
}

fn extent_to_nav_grid_bounds(
    extent: ChunkExtent,
    layout: ChunkLayout,
    nav_config: NavigationConfig,
) -> (GridCoord, GridCoord) {
    let size = layout.chunk_size_meters;
    let min_g = Vec3::new(extent.min.x as f32 * size, 0.0, extent.min.z as f32 * size);
    let max_g = Vec3::new(
        (extent.max.x as f32 + 1.0) * size - 0.01,
        0.0,
        (extent.max.z as f32 + 1.0) * size - 0.01,
    );
    (
        grid_coord_at_global_xz(min_g, nav_config),
        grid_coord_at_global_xz(max_g, nav_config),
    )
}

/// Sample pathing-mask cells across the whole navigation world bounds.
///
/// Skips cells with no terrain grounding. Does not require a unit/building selection.
pub fn sample_navigation_mask_cells(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: NavigationConfig,
) -> Vec<NavigationMaskCellSample> {
    let Some((min, max)) = navigation_mask_world_grid_bounds(world, nav_config) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for z in min.z..=max.z {
        for x in min.x..=max.x {
            let coord = GridCoord::new(x, z);
            let Some(position) = crate::world::grid_cell_world_position(world, coord, nav_config)
            else {
                continue;
            };
            let cell_center = grid_cell_center_global(coord, nav_config);
            let walkable = is_cell_walkable(world, catalogs, nav_config, DEBUG_NAV_AGENT, coord);
            let block_reason = if walkable {
                None
            } else {
                match query_passability_at(
                    world,
                    catalogs,
                    position,
                    PassabilityAgent::from(DEBUG_NAV_AGENT),
                ) {
                    PassabilityResult::Blocked { reason, .. } => Some(reason),
                    _ => Some(PassabilityBlockReason::InvalidCell),
                }
            };
            out.push(NavigationMaskCellSample {
                coord,
                center_xz: Vec2::new(cell_center.x, cell_center.z),
                walkable,
                block_reason,
            });
        }
    }
    out
}

fn refresh_navigation_pathing_mask_cache(
    world: &WorldData,
    catalogs: &PassabilityCatalogs<'_>,
    nav_config: NavigationConfig,
    cache: &mut NavigationMaskCache,
) -> NavigationMaskDrawStats {
    let resident = world.len();
    let needs_rebuild = !cache.active
        || cache.samples.is_empty()
        || cache.resident_chunk_count != resident
        || cache.frames_since_rebuild >= MASK_CACHE_REFRESH_FRAMES;
    let first_activation = !cache.active;
    if needs_rebuild {
        cache.samples = sample_navigation_mask_cells(world, *catalogs, nav_config);
        cache.resident_chunk_count = resident;
        cache.frames_since_rebuild = 0;
        cache.active = !cache.samples.is_empty();
        cache.cell_spacing_meters = nav_config.cell_spacing_meters;
        cache.revision = cache.revision.saturating_add(1);
        if first_activation && cache.active {
            bevy::log::info!(
                "nav pathing mask: sampled={} navigable={} blocked={}",
                cache.samples.len(),
                cache.samples.iter().filter(|s| s.walkable).count(),
                cache.samples.iter().filter(|s| !s.walkable).count()
            );
        }
    } else {
        cache.frames_since_rebuild = cache.frames_since_rebuild.saturating_add(1);
    }

    NavigationMaskDrawStats {
        ran: true,
        cells_sampled: cache.samples.len() as u32,
        ..Default::default()
    }
}

pub(crate) fn block_reason_color(reason: PassabilityBlockReason) -> Color {
    match reason {
        PassabilityBlockReason::SlopeTooSteep => Color::srgba(0.95, 0.55, 0.1, 0.82),
        PassabilityBlockReason::BuildingOccupied => Color::srgba(0.95, 0.15, 0.15, 0.85),
        PassabilityBlockReason::DoodadOccupied => Color::srgba(0.75, 0.2, 0.95, 0.85),
        PassabilityBlockReason::CorruptFootprint => Color::srgba(0.5, 0.1, 0.1, 0.8),
        PassabilityBlockReason::MissingDefinition => Color::srgba(0.4, 0.4, 0.4, 0.8),
        PassabilityBlockReason::InvalidCell => Color::srgba(0.25, 0.25, 0.28, 0.7),
    }
}

fn draw_occupancy_cells(
    gizmos: &mut Gizmos,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    focus: Vec3,
    draw_blocked: bool,
    draw_reserved: bool,
) {
    let cell_size = crate::world::OCCUPANCY_CELL_SIZE_METERS;
    let half = cell_size * 0.48;
    let radius_sq = SPECIALIZED_OVERLAY_RADIUS_METERS * SPECIALIZED_OVERLAY_RADIUS_METERS;
    let mut drawn = 0_u32;

    'chunks: for (chunk_id, grid) in world.occupancy_grids() {
        if !chunk_near_focus(*chunk_id, focus, layout) {
            continue;
        }
        for (cell, entry) in grid.cells() {
            if drawn >= MAX_OCCUPANCY_CELLS_DRAWN {
                break 'chunks;
            }
            let center = cell.center_global();
            let dx = center.x - focus.x;
            let dz = center.y - focus.z;
            if dx * dx + dz * dz > radius_sq {
                continue;
            }
            let color = match entry.state {
                OccupancyState::Blocked if draw_blocked => Color::srgba(0.9, 0.2, 0.2, 0.65),
                OccupancyState::Reserved if draw_reserved => Color::srgba(0.95, 0.85, 0.15, 0.7),
                _ => continue,
            };
            draw_xz_quad(
                gizmos,
                world,
                layout,
                vertical_scale,
                center,
                half,
                0.05,
                color,
            );
            drawn += 1;
        }
    }
}

fn chunk_near_focus(chunk_id: ChunkId, focus: Vec3, layout: ChunkLayout) -> bool {
    let chunk_size = layout.chunk_size_meters;
    let origin = Vec3::new(
        chunk_id.coord().x as f32 * chunk_size,
        0.0,
        chunk_id.coord().z as f32 * chunk_size,
    );
    let center = origin + Vec3::splat(chunk_size * 0.5);
    center.distance(focus) <= SPECIALIZED_OVERLAY_RADIUS_METERS + chunk_size
}

fn draw_building_footprints(
    gizmos: &mut Gizmos,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    focus: Vec3,
    selected_building: Option<crate::world::BuildingId>,
) {
    let radius_sq = SPECIALIZED_OVERLAY_RADIUS_METERS * SPECIALIZED_OVERLAY_RADIUS_METERS;
    for building_id in world.sorted_building_ids() {
        let Some(building) = world.get_building(building_id) else {
            continue;
        };
        let global = building.placement.position.to_global(layout);
        let dx = global.x - focus.x;
        let dz = global.z - focus.z;
        if dx * dx + dz * dz > radius_sq {
            continue;
        }
        let Some(definition) = building_catalog.get(&building.definition_id) else {
            continue;
        };
        let Ok(shape) = effective_building_footprint_for_placement(
            definition,
            footprint_catalog,
            building.placement.uniform_scale_f32(),
        ) else {
            continue;
        };
        let anchor_xz = Vec2::new(global.x, global.z);
        let yaw = building.placement.rotation.to_euler(EulerRot::YXZ).0;
        let cells = occupied_cells_for_footprint_yaw(shape.as_ref(), anchor_xz, yaw);
        let highlight = selected_building == Some(building_id);
        let color = if highlight {
            Color::srgba(0.2, 0.75, 1.0, 0.85)
        } else {
            Color::srgba(0.35, 0.55, 0.95, 0.55)
        };
        for cell in cells {
            draw_xz_quad(
                gizmos,
                world,
                layout,
                vertical_scale,
                cell.center_global(),
                crate::world::OCCUPANCY_CELL_SIZE_METERS * 0.48,
                0.07,
                color,
            );
        }
    }
}

fn draw_portal_markers(
    gizmos: &mut Gizmos,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    focus: Vec3,
    selected_building: Option<crate::world::BuildingId>,
) {
    let radius_sq = SPECIALIZED_OVERLAY_RADIUS_METERS * SPECIALIZED_OVERLAY_RADIUS_METERS;
    for (_id, portal) in world.space_registry().portals() {
        if !portal.enabled {
            continue;
        }
        let center_xz = portal.from_center_global_xz;
        let dx = center_xz.x - focus.x;
        let dz = center_xz.y - focus.z;
        if dx * dx + dz * dz > radius_sq {
            continue;
        }
        let highlight = selected_building.is_some_and(|id| portal.owning_building_id == Some(id));
        let (ring_color, fill_color) = portal_colors(portal.portal_type, highlight);
        let y = super::nav_cells::sample_terrain_y(world, center_xz, layout, vertical_scale) + 0.12;
        let center = Vec3::new(center_xz.x, y, center_xz.y);
        gizmos.circle(
            Isometry3d::new(center, Quat::IDENTITY),
            portal.from_radius_meters,
            ring_color,
        );
        gizmos.sphere(center, portal.from_radius_meters * 0.15, fill_color);

        let dest = render_position(portal.to_position, layout, vertical_scale);
        gizmos.line(
            xz_to_render_y(center, 0.1),
            xz_to_render_y(dest, 0.15),
            Color::srgba(0.4, 0.9, 1.0, 0.6),
        );
    }
}

fn portal_colors(portal_type: PortalType, highlight: bool) -> (Color, Color) {
    if highlight {
        return (
            Color::srgba(0.2, 1.0, 1.0, 0.95),
            Color::srgba(0.2, 1.0, 1.0, 0.5),
        );
    }
    match portal_type {
        PortalType::ExteriorEntrance => (
            Color::srgba(0.2, 0.9, 0.95, 0.85),
            Color::srgba(0.2, 0.9, 0.95, 0.4),
        ),
        PortalType::Doorway => (
            Color::srgba(0.55, 0.75, 1.0, 0.8),
            Color::srgba(0.55, 0.75, 1.0, 0.35),
        ),
        PortalType::Stair | PortalType::Ramp => (
            Color::srgba(0.7, 0.55, 0.95, 0.8),
            Color::srgba(0.7, 0.55, 0.95, 0.35),
        ),
        PortalType::CaveEntrance => (
            Color::srgba(0.55, 0.4, 0.3, 0.8),
            Color::srgba(0.55, 0.4, 0.3, 0.35),
        ),
    }
}

/// Draw activated blueprint floor outlines from the runtime navigation store (NV1.3).
fn draw_runtime_blueprint_floors(
    gizmos: &mut Gizmos,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    focus: Vec3,
    active_space: Option<SpaceId>,
    selected_building: Option<crate::world::BuildingId>,
) {
    let radius_sq = SPECIALIZED_OVERLAY_RADIUS_METERS * SPECIALIZED_OVERLAY_RADIUS_METERS;
    for runtime in world.building_navigation_runtime().iter() {
        if selected_building.is_some_and(|id| id != runtime.building_id) {
            continue;
        }
        let building_global = world
            .get_building(runtime.building_id)
            .map(|record| record.placement.position.to_global(layout))
            .unwrap_or(focus);
        let dx = building_global.x - focus.x;
        let dz = building_global.z - focus.z;
        if dx * dx + dz * dz > radius_sq {
            continue;
        }

        for floor in &runtime.floors {
            if floor.world_outline_xz.len() < 2 {
                continue;
            }
            let active = active_space == Some(floor.space_id);
            let floor_y = world
                .space_registry()
                .get_space(floor.space_id)
                .map(|space| space.floor_y_global)
                .unwrap_or(floor.elevation_meters);
            let edge_color = if active {
                Color::srgba(1.0, 0.85, 0.15, 0.95)
            } else {
                Color::srgba(0.55, 0.35, 0.95, 0.75)
            };
            let fill_color = Color::srgba(0.55, 0.35, 0.95, if active { 0.2 } else { 0.08 });

            let verts: Vec<Vec3> = floor
                .world_outline_xz
                .iter()
                .map(|xz| {
                    render_position(
                        crate::world::WorldPosition::from_global(
                            Vec3::new(xz.x, floor_y, xz.y),
                            layout,
                        ),
                        layout,
                        vertical_scale,
                    )
                })
                .collect();

            for i in 0..verts.len() {
                let a = verts[i];
                let b = verts[(i + 1) % verts.len()];
                gizmos.line(a, b, edge_color);
                if active {
                    gizmos.line(a, verts[(i + 2) % verts.len()], fill_color);
                }
            }
            if active {
                let centroid =
                    verts.iter().fold(Vec3::ZERO, |acc, v| acc + *v) / verts.len() as f32;
                gizmos.sphere(centroid, 0.2, Color::srgba(1.0, 0.85, 0.15, 0.9));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug::settings::{
        DebugOverlayCategory, DebugOverlayConfig, debug_navigation_overlay_enabled,
    };
    use crate::world::{
        BuildingCatalog, ChunkCoord, ChunkData, ChunkId, DoodadCatalog, FootprintCatalog,
        Heightfield, LocalPosition, WorldData, WorldPosition,
    };

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

    fn catalogs() -> (DoodadCatalog, BuildingCatalog, FootprintCatalog) {
        (
            DoodadCatalog::default(),
            BuildingCatalog::default(),
            FootprintCatalog::default(),
        )
    }

    #[test]
    fn pathing_mask_toggle_updates_authoritative_debug_resource() {
        let mut config = DebugOverlayConfig::production();
        assert!(!config.grid);
        assert!(!debug_navigation_overlay_enabled(&config));

        config.enabled = true;
        config.grid = true;
        assert!(config.category_enabled(DebugOverlayCategory::Grid));
        assert!(config.navigation_overlay_active());
        assert!(debug_navigation_overlay_enabled(&config));

        config.grid = false;
        assert!(!config.navigation_overlay_active());
    }

    #[test]
    fn pathing_mask_run_condition_requires_master_overlay() {
        let config = DebugOverlayConfig {
            enabled: false,
            grid: true,
            ..DebugOverlayConfig::production()
        };
        assert!(!debug_navigation_overlay_enabled(&config));
    }

    #[test]
    fn mask_sampling_does_not_require_selection() {
        let world = flat_world();
        let (doodad, building, footprint) = catalogs();
        let pass = PassabilityCatalogs {
            doodad: &doodad,
            building: &building,
            footprint: &footprint,
        };
        let samples = sample_navigation_mask_cells(&world, pass, NavigationConfig::default());
        assert!(
            !samples.is_empty(),
            "expected grounded cells across resident world"
        );
        assert!(
            samples.iter().any(|s| s.walkable),
            "flat empty terrain should yield navigable cells"
        );
    }

    #[test]
    fn mask_sampling_classifies_navigable_and_blocked_cells() {
        let mut world = flat_world();
        // Steep ridge (center peak) so some cells fail the 45° debug agent slope check.
        // Adjacent samples are 128 m apart; height delta > 128 m exceeds 45°.
        let mut samples = vec![0.0_f32; 9];
        samples[4] = 200.0;
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );

        let (doodad, building, footprint) = catalogs();
        let pass = PassabilityCatalogs {
            doodad: &doodad,
            building: &building,
            footprint: &footprint,
        };
        let mask = sample_navigation_mask_cells(&world, pass, NavigationConfig::default());
        let navigable = mask.iter().filter(|s| s.walkable).count();
        let blocked = mask.iter().filter(|s| !s.walkable).count();
        assert!(navigable > 0, "expected some navigable cells");
        assert!(blocked > 0, "expected some blocked cells on steep terrain");
        assert!(
            mask.iter().any(|s| !s.walkable && s.block_reason.is_some()),
            "blocked cells should carry a passability reason"
        );
    }

    #[test]
    fn mask_bounds_cover_resident_extent_without_focus_radius() {
        let world = flat_world();
        let (min, max) =
            navigation_mask_world_grid_bounds(&world, NavigationConfig::default()).unwrap();
        let spacing = NavigationConfig::default().cell_spacing_meters;
        let width_m = (max.x - min.x + 1) as f32 * spacing;
        let depth_m = (max.z - min.z + 1) as f32 * spacing;
        assert!(
            width_m >= 200.0 && depth_m >= 200.0,
            "mask bounds should span the resident chunk (~256m), got {width_m}x{depth_m}"
        );
    }

    #[test]
    fn disabling_mask_clears_active_cache_flag_semantics() {
        let mut cache = NavigationMaskCache {
            samples: vec![NavigationMaskCellSample {
                coord: GridCoord::new(0, 0),
                center_xz: Vec2::ZERO,
                walkable: true,
                block_reason: None,
            }],
            resident_chunk_count: 1,
            frames_since_rebuild: 0,
            active: true,
            revision: 1,
            cell_spacing_meters: 4.0,
        };
        let settings = DebugOverlayConfig::production();
        assert!(!settings.navigation_overlay_active());
        // Mimic overlay early-out when inactive.
        if !settings.navigation_overlay_active() {
            cache.active = false;
            cache.samples.clear();
        }
        assert!(!cache.active);
        assert!(cache.samples.is_empty());
    }

    #[test]
    fn mask_state_is_not_part_of_world_position_payload() {
        // NavigationMaskCache / DrawStats are debug resources only; WorldPosition
        // serialization must not embed overlay toggles.
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(1.0, 0.0, 2.0)),
        );
        let encoded = format!("{position:?}");
        assert!(!encoded.contains("NavigationMask"));
        assert!(!encoded.contains("nav_blockers"));
    }

    #[test]
    fn pathing_mask_does_not_enable_unit_path_overlay() {
        let config = DebugOverlayConfig {
            enabled: true,
            grid: true,
            ..DebugOverlayConfig::production()
        };
        assert!(config.navigation_overlay_active());
        assert!(!config.path);
        assert!(!config.category_enabled(DebugOverlayCategory::Path));
    }
}
