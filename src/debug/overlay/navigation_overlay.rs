//! Navigation debug overlay — walkability, occupancy, portals, footprints (NV0).
//!
//! Observes navigation and occupancy state only; does not mutate simulation.

use bevy::prelude::*;

use crate::camera::RtsCameraState;
use crate::dev::WorldInspectorState;
use crate::debug::settings::DebugOverlaySettings;
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::primary_selected_unit;
use crate::units::input::SelectedUnits;
use crate::world::{
    BuildingCatalog, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog,
    GridCoord, NavigationAgent, NavigationConfig, OccupancyState, PassabilityAgent,
    PassabilityBlockReason, PassabilityCatalogs, PassabilityResult, PortalType, SpaceId,
    WorldConfig, WorldData, effective_building_footprint_for_placement, grid_cell_center_global,
    grid_coord_at_global_xz, is_cell_walkable, occupied_cells_for_footprint_yaw,
    query_passability_at,
};

use super::helpers::{render_position, xz_to_render_y};
use super::nav_cells::draw_xz_quad;

const NAV_DEBUG_RADIUS_METERS: f32 = 48.0;
const MAX_NAV_CELLS_DRAWN: u32 = 400;
const MAX_OCCUPANCY_CELLS_DRAWN: u32 = 600;

/// Default agent used when sampling walkability near the camera focus.
const DEBUG_NAV_AGENT: NavigationAgent = NavigationAgent {
    radius_meters: 0.5,
    max_slope_degrees: 45.0,
};

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
    inspector: Res<WorldInspectorState>,
    camera: Query<&RtsCameraState, With<crate::camera::RtsCamera>>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.navigation_overlay_active() {
        return;
    }

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
    let focus = debug_focus_global(&world, &selection, &inspector, &camera, layout);

    if settings.grid || settings.nav_blockers {
        draw_navigation_cells(
            &mut gizmos,
            &world,
            layout,
            vertical_scale,
            &catalogs,
            *nav_config,
            focus,
            settings.grid,
            settings.nav_blockers,
        );
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
            inspector.selected_building,
        );
    }

    if settings.nav_entrances {
        draw_portal_markers(
            &mut gizmos,
            &world,
            layout,
            vertical_scale,
            focus,
            inspector.selected_building,
        );
    }

    if settings.nav_blueprint || settings.nav_entrances || settings.nav_footprints {
        let active_space = inspector
            .selected_unit
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
            inspector.selected_building,
        );
    }
}

fn debug_focus_global(
    world: &WorldData,
    selection: &SelectedUnits,
    inspector: &WorldInspectorState,
    camera: &Query<&RtsCameraState, With<crate::camera::RtsCamera>>,
    layout: ChunkLayout,
) -> Vec3 {
    if let Some(unit_id) = inspector
        .selected_unit
        .or_else(|| primary_selected_unit(selection))
    {
        if let Some(unit) = world.get_unit(unit_id) {
            return unit.placement.position.to_global(layout);
        }
    }
    if let Some(building_id) = inspector.selected_building {
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

fn draw_navigation_cells(
    gizmos: &mut Gizmos,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    catalogs: &PassabilityCatalogs<'_>,
    nav_config: NavigationConfig,
    focus: Vec3,
    draw_walkable: bool,
    draw_blockers: bool,
) {
    let spacing = nav_config.cell_spacing_meters;
    let radius_cells = (NAV_DEBUG_RADIUS_METERS / spacing).ceil() as i32;
    let center = grid_coord_at_global_xz(focus, nav_config);
    let half = spacing * 0.45;
    let mut drawn = 0_u32;

    'cells: for dz in -radius_cells..=radius_cells {
        for dx in -radius_cells..=radius_cells {
            if drawn >= MAX_NAV_CELLS_DRAWN {
                break 'cells;
            }
            let coord = GridCoord::new(center.x + dx, center.z + dz);
            let cell_center = grid_cell_center_global(coord, nav_config);
            if cell_center.distance(focus) > NAV_DEBUG_RADIUS_METERS {
                continue;
            }
            let walkable = is_cell_walkable(world, *catalogs, nav_config, DEBUG_NAV_AGENT, coord);
            if walkable {
                if draw_walkable {
                    draw_xz_quad(
                        gizmos,
                        world,
                        layout,
                        vertical_scale,
                        Vec2::new(cell_center.x, cell_center.z),
                        half,
                        0.06,
                        Color::srgba(0.15, 0.85, 0.35, 0.55),
                    );
                    drawn += 1;
                }
                continue;
            }
            if !draw_blockers {
                continue;
            }
            let Some(position) =
                crate::world::grid_cell_world_position(world, coord, nav_config)
            else {
                continue;
            };
            let color = passability_block_color(query_passability_at(
                world,
                *catalogs,
                position,
                PassabilityAgent::from(DEBUG_NAV_AGENT),
            ));
            draw_xz_quad(
                gizmos,
                world,
                layout,
                vertical_scale,
                Vec2::new(cell_center.x, cell_center.z),
                half,
                0.08,
                color,
            );
            drawn += 1;
        }
    }
}

fn passability_block_color(result: PassabilityResult) -> Color {
    match result {
        PassabilityResult::Passable { .. } => Color::srgba(0.15, 0.85, 0.35, 0.55),
        PassabilityResult::Blocked { reason, .. } => match reason {
            PassabilityBlockReason::SlopeTooSteep => Color::srgba(0.95, 0.55, 0.1, 0.7),
            PassabilityBlockReason::BuildingOccupied => Color::srgba(0.95, 0.15, 0.15, 0.75),
            PassabilityBlockReason::DoodadOccupied => Color::srgba(0.75, 0.2, 0.95, 0.75),
            PassabilityBlockReason::CorruptFootprint => Color::srgba(0.5, 0.1, 0.1, 0.7),
            PassabilityBlockReason::MissingDefinition => Color::srgba(0.4, 0.4, 0.4, 0.7),
            PassabilityBlockReason::InvalidCell => Color::srgba(0.3, 0.3, 0.3, 0.7),
        },
        PassabilityResult::Unavailable { .. } => Color::srgba(0.2, 0.2, 0.25, 0.5),
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
    let radius_sq = NAV_DEBUG_RADIUS_METERS * NAV_DEBUG_RADIUS_METERS;
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
                OccupancyState::Blocked if draw_blocked => {
                    Color::srgba(0.9, 0.2, 0.2, 0.65)
                }
                OccupancyState::Reserved if draw_reserved => {
                    Color::srgba(0.95, 0.85, 0.15, 0.7)
                }
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
    center.distance(focus) <= NAV_DEBUG_RADIUS_METERS + chunk_size
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
    let radius_sq = NAV_DEBUG_RADIUS_METERS * NAV_DEBUG_RADIUS_METERS;
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
    let radius_sq = NAV_DEBUG_RADIUS_METERS * NAV_DEBUG_RADIUS_METERS;
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
    let radius_sq = NAV_DEBUG_RADIUS_METERS * NAV_DEBUG_RADIUS_METERS;
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
                let centroid = verts.iter().fold(Vec3::ZERO, |acc, v| acc + *v) / verts.len() as f32;
                gizmos.sphere(centroid, 0.2, Color::srgba(1.0, 0.85, 0.15, 0.9));
            }
        }
    }
}
