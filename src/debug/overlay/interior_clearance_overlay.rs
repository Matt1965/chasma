//! Interior region clearance diagnostic overlay (IN-11d).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::debug::settings::DebugOverlaySettings;
use crate::dev::BlueprintInspectionState;
use crate::terrain::TerrainRenderAssets;
use crate::world::{
    BuildingCatalog, BuildingNavigationBlueprintCatalog, DoodadCatalog, FootprintCatalog,
    NavigationAgent, NavigationConfig, PassabilityCatalogs, WorldConfig, WorldData, WorldPosition,
    building_model_render_transform, cell_walkability_sample_globals,
    inset_polygon_toward_centroid, measure_interior_region_clearance,
    resolve_building_navigation_blueprint, signed_distance_to_polygon_edges,
};

use super::helpers::{closed_polygon_boundary_segments, xz_to_render_y};
use super::nav_cells::draw_xz_quad;

const Y_LIFT: f32 = 0.12;
const AUTHORED_COLOR: Color = Color::srgba(0.95, 0.85, 0.2, 0.95);
const RUNTIME_COLOR: Color = Color::srgba(0.2, 0.85, 1.0, 0.95);
const INSET_COLOR: Color = Color::srgba(0.2, 1.0, 0.55, 0.75);
const PASS_CELL: Color = Color::srgba(0.15, 0.85, 0.35, 0.55);
const FAIL_CELL: Color = Color::srgba(0.95, 0.25, 0.2, 0.55);
const SAMPLE_COLOR: Color = Color::srgba(1.0, 1.0, 0.3, 0.9);
const PORTAL_COLOR: Color = Color::srgba(1.0, 0.45, 0.1, 0.95);
const GOAL_COLOR: Color = Color::srgba(0.45, 0.65, 1.0, 0.95);
const ROBOT_COLOR: Color = Color::srgba(1.0, 0.2, 0.8, 0.85);

const DEBUG_AGENT: NavigationAgent = NavigationAgent {
    radius_meters: 0.6,
    max_slope_degrees: 45.0,
};

pub fn draw_interior_clearance_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    nav_config: Res<NavigationConfig>,
    building_catalog: Res<BuildingCatalog>,
    nav_catalog: Res<BuildingNavigationBlueprintCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    settings: Res<DebugOverlaySettings>,
    inspection: Res<BlueprintInspectionState>,
    world_selection: Res<WorldSelectionState>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.nav_clearance {
        return;
    }

    let building_id = inspection.building_id.or((world_selection.category
        == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten());
    let Some(building_id) = building_id else {
        return;
    };
    let Some(record) = world.get_building(building_id) else {
        return;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return;
    };
    let Some(runtime) = world.building_navigation_runtime().get(building_id) else {
        return;
    };

    let region_key = inspection
        .selected_region_key
        .as_deref()
        .or(runtime.regions.first().map(|r| r.region_key.as_str()));
    let Some(region_key) = region_key else {
        return;
    };
    let Some(region) = runtime
        .regions
        .iter()
        .find(|region| region.region_key == region_key)
    else {
        return;
    };

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let model_transform =
        building_model_render_transform(definition, &record.placement, layout, vertical_scale);
    let floor_y = world
        .space_registry()
        .get_space(region.space_id)
        .map(|space| space.floor_y_global * vertical_scale)
        .unwrap_or(model_transform.translation.y);

    let blueprint_owned = inspection.working_copy.clone().or_else(|| {
        resolve_building_navigation_blueprint(
            definition,
            &nav_catalog,
            record.interior.navigation_blueprint_override.as_ref(),
        )
        .ok()
        .flatten()
        .map(|resolved| resolved.blueprint().clone())
    });

    let blueprint_local = blueprint_owned
        .as_ref()
        .and_then(|bp| {
            bp.floors
                .iter()
                .find(|floor| floor.floor_id == region.floor_id)
                .and_then(|floor| floor.region_by_key(region_key))
                .map(|authored| {
                    authored
                        .walkable_outline
                        .vertices_xz
                        .iter()
                        .map(|v| Vec2::new(v[0], v[1]))
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or_default();

    let portal_world = blueprint_owned
        .as_ref()
        .and_then(|bp| bp.entrances.first())
        .map(|entrance| interior_spawn_world(&runtime, entrance, floor_y, vertical_scale, layout));
    let goal_world = portal_world.map(|_| {
        let interior = runtime
            .model_transform
            .transform_point(Vec3::new(0.0, 0.0, 1.0));
        WorldPosition::from_global(
            Vec3::new(interior.x, floor_y / vertical_scale, interior.z),
            layout,
        )
    });

    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint_catalog,
    };

    let report = measure_interior_region_clearance(
        &world,
        world.space_registry(),
        world.building_navigation_runtime(),
        catalogs,
        &nav_config,
        DEBUG_AGENT,
        region.space_id,
        &blueprint_local,
        portal_world,
        goal_world,
    );

    draw_authored_polygon(&mut gizmos, &blueprint_local, &model_transform, floor_y);
    draw_runtime_polygon(
        &mut gizmos,
        &region.world_outline_xz,
        floor_y,
        RUNTIME_COLOR,
    );

    let inset = inset_polygon_toward_centroid(&region.world_outline_xz, DEBUG_AGENT.radius_meters);
    draw_runtime_polygon(&mut gizmos, &inset, floor_y, INSET_COLOR);

    if let Some(report) = report {
        let space_config = nav_config.config_for_space(region.space_id);
        let half = space_config.cell_spacing_meters * 0.5;
        for probe in &report.cell_probes {
            let color = if probe.permissive_pass {
                PASS_CELL
            } else {
                FAIL_CELL
            };
            draw_xz_quad(
                &mut gizmos,
                &world,
                layout,
                vertical_scale,
                probe.center_global_xz,
                half * 0.45,
                Y_LIFT + 0.04,
                color,
            );
            gizmos.sphere(
                Vec3::new(
                    probe.center_global_xz.x,
                    floor_y + Y_LIFT + 0.08,
                    probe.center_global_xz.y,
                ),
                0.06,
                SAMPLE_COLOR,
            );
        }

        for probe in &report.cell_probes {
            let samples = cell_walkability_sample_globals(
                probe.coord,
                space_config,
                DEBUG_AGENT.radius_meters,
            );
            for sample in samples {
                gizmos.sphere(
                    Vec3::new(sample.x, floor_y + Y_LIFT + 0.1, sample.z),
                    0.04,
                    SAMPLE_COLOR,
                );
            }
        }

        if let Some(landing) = portal_world {
            let landing_xz = landing.to_global(layout).xz();
            draw_endpoint(&mut gizmos, landing_xz, floor_y, PORTAL_COLOR);
            draw_clearance_line(&mut gizmos, landing_xz, &region.world_outline_xz, floor_y);
        }
        if let Some(goal) = goal_world {
            let goal_xz = goal.to_global(layout).xz();
            draw_endpoint(&mut gizmos, goal_xz, floor_y, GOAL_COLOR);
            draw_clearance_line(&mut gizmos, goal_xz, &region.world_outline_xz, floor_y);
            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(goal_xz.x, floor_y + Y_LIFT + 0.14, goal_xz.y),
                    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                ),
                DEBUG_AGENT.radius_meters,
                ROBOT_COLOR,
            );
        }
    }
}

fn interior_spawn_world(
    runtime: &crate::world::BuildingNavigationRuntime,
    entrance: &crate::world::NavigationEntranceDefinition,
    floor_y: f32,
    vertical_scale: f32,
    layout: crate::world::ChunkLayout,
) -> WorldPosition {
    let local = Vec3::new(
        entrance.interior_spawn_local[0],
        entrance.interior_spawn_local[1],
        entrance.interior_spawn_local[2],
    );
    let global = runtime.model_transform.transform_point(local);
    WorldPosition::from_global(
        Vec3::new(global.x, floor_y / vertical_scale, global.z),
        layout,
    )
}

fn draw_authored_polygon(
    gizmos: &mut Gizmos,
    blueprint_local: &[Vec2],
    model_transform: &Transform,
    floor_y: f32,
) {
    if blueprint_local.len() < 2 {
        return;
    }
    let vertices: Vec<Vec3> = blueprint_local
        .iter()
        .map(|vertex| {
            let world = model_transform.transform_point(Vec3::new(vertex.x, 0.0, vertex.y));
            Vec3::new(world.x, floor_y + Y_LIFT, world.z)
        })
        .collect();
    for (a, b) in closed_polygon_boundary_segments(vertices.len()) {
        gizmos.line(vertices[a], vertices[b], AUTHORED_COLOR);
    }
}

fn draw_runtime_polygon(gizmos: &mut Gizmos, polygon: &[Vec2], floor_y: f32, color: Color) {
    if polygon.len() < 2 {
        return;
    }
    let vertices: Vec<Vec3> = polygon
        .iter()
        .map(|vertex| Vec3::new(vertex.x, floor_y + Y_LIFT, vertex.y))
        .collect();
    for (a, b) in closed_polygon_boundary_segments(vertices.len()) {
        gizmos.line(vertices[a], vertices[b], color);
    }
}

fn draw_endpoint(gizmos: &mut Gizmos, xz: Vec2, floor_y: f32, color: Color) {
    gizmos.sphere(Vec3::new(xz.x, floor_y + Y_LIFT + 0.14, xz.y), 0.12, color);
}

fn draw_clearance_line(gizmos: &mut Gizmos, point: Vec2, polygon: &[Vec2], floor_y: f32) {
    if signed_distance_to_polygon_edges(point, polygon) >= 0.0 {
        return;
    }
    let mut nearest = point;
    let mut min_dist = f32::INFINITY;
    let count = polygon.len();
    for index in 0..count {
        let a = polygon[index];
        let b = polygon[(index + 1) % count];
        let edge = b - a;
        let len_sq = edge.length_squared();
        if len_sq <= f32::EPSILON {
            continue;
        }
        let t = ((point - a).dot(edge) / len_sq).clamp(0.0, 1.0);
        let closest = a + edge * t;
        let dist = point.distance(closest);
        if dist < min_dist {
            min_dist = dist;
            nearest = closest;
        }
    }
    let start = Vec3::new(point.x, floor_y + Y_LIFT + 0.2, point.y);
    let end = Vec3::new(nearest.x, floor_y + Y_LIFT + 0.2, nearest.y);
    gizmos.line(start, end, Color::srgba(1.0, 1.0, 0.5, 0.9));
    gizmos.sphere(
        (start + end) * 0.5 + Vec3::Y * 0.05,
        0.05,
        Color::srgba(1.0, 1.0, 0.5, 0.9),
    );
}
