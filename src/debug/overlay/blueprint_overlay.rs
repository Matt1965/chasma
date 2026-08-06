//! Generated navigation blueprint overlay (NV1.2.5). Read-only blueprint geometry.

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::terrain::TerrainRenderAssets;
use crate::world::{
    BlueprintDiagnosticFocus, BuildingCatalog, BuildingNavigationBlueprint,
    BuildingNavigationBlueprintCatalog, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationVerticalTransitionDefinition, WorldConfig, WorldData,
    building_model_render_transform, resolve_building_navigation_blueprint,
};

use super::helpers::{closed_polygon_boundary_segments, xz_to_render_y};
use crate::debug::InspectorOverlayFocus;
use crate::debug::settings::DebugOverlaySettings;
use crate::dev::{
    BlueprintEditSelection, BlueprintInspectionState, WorldInspectorState, blueprint_local_to_world,
};

const FLOOR_Y_OFFSET: f32 = 0.08;
const OTHER_FLOOR_ALPHA: f32 = 0.22;
const VERTEX_GIZMO_RADIUS: f32 = 0.18;

pub fn draw_blueprint_debug_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    building_catalog: Res<BuildingCatalog>,
    nav_catalog: Res<BuildingNavigationBlueprintCatalog>,
    settings: Res<DebugOverlaySettings>,
    inspection: Res<BlueprintInspectionState>,
    inspector: Res<WorldInspectorState>,
    world_selection: Res<WorldSelectionState>,
    overlay_focus: Res<InspectorOverlayFocus>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.blueprint_overlay_active() {
        return;
    }

    let building_id = inspection
        .building_id
        .or(
            (world_selection.category == WorldSelectionCategory::Building)
                .then_some(world_selection.building_id)
                .flatten(),
        )
        .or(overlay_focus.blueprint_building_id);
    let Some(building_id) = building_id else {
        return;
    };

    let Some(record) = world.get_building(building_id) else {
        return;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return;
    };

    let blueprint_owned = if inspection.editing || inspection.working_copy.is_some() {
        None
    } else {
        match resolve_building_navigation_blueprint(
            definition,
            &nav_catalog,
            record.interior.navigation_blueprint_override.as_ref(),
        ) {
            Ok(Some(resolved)) => Some(resolved.blueprint().clone()),
            _ => None,
        }
    };
    let previewing_unadopted_draft = inspection.draft_preview_active
        && inspection
            .generated_draft
            .as_ref()
            .is_some_and(|draft| !draft.adopted);
    let working_for_overlay = inspection.working_copy.as_ref().filter(|working| {
        !previewing_unadopted_draft || working.floors.iter().any(|floor| !floor.regions.is_empty())
    });
    let blueprint = working_for_overlay.or(blueprint_owned.as_ref());
    let Some(blueprint) = blueprint else {
        return;
    };

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let transform =
        building_model_render_transform(definition, &record.placement, layout, vertical_scale);

    let selected_floor = resolve_overlay_floor_id(blueprint, &inspection, &overlay_focus);
    let diagnostic = overlay_focus.blueprint_diagnostic.as_ref().or(inspection
        .focused_diagnostic_index
        .and_then(|index| {
            inspector
                .blueprint_snapshot
                .as_ref()
                .and_then(|snap| snap.validation.diagnostics.get(index))
                .and_then(|d| d.focus.as_ref())
        }));

    draw_building_origin(&mut gizmos, &transform);

    for floor in &blueprint.floors {
        let emphasized = selected_floor
            .map(|id| id == floor.floor_id)
            .unwrap_or(true);
        let alpha = if emphasized { 1.0 } else { OTHER_FLOOR_ALPHA };
        draw_floor_polygon(
            &mut gizmos,
            &transform,
            floor,
            alpha,
            diagnostic,
            emphasized,
        );
    }

    for entrance in &blueprint.entrances {
        let floor_id = blueprint
            .floors
            .iter()
            .find(|f| f.key == entrance.floor_key)
            .map(|f| f.floor_id);
        let emphasized = selected_floor
            .zip(floor_id)
            .map(|(a, b)| a == b)
            .unwrap_or(true);
        if !emphasized {
            continue;
        }
        draw_entrance(
            &mut gizmos,
            &transform,
            entrance,
            floor_elevation(blueprint, &entrance.floor_key),
            diagnostic,
            blueprint,
        );
    }

    for transition in &blueprint.vertical_transitions {
        let from_floor = blueprint
            .floors
            .iter()
            .find(|f| f.key == transition.from_floor_key);
        let emphasized = selected_floor
            .zip(from_floor.map(|f| f.floor_id))
            .map(|(a, b)| a == b)
            .unwrap_or(true);
        if !emphasized {
            continue;
        }
        draw_vertical_transition(&mut gizmos, &transform, transition, blueprint, diagnostic);
    }

    for connection in &blueprint.region_connections {
        let floor = blueprint
            .floors
            .iter()
            .find(|f| f.key == connection.floor_key);
        let emphasized = selected_floor
            .zip(floor.map(|f| f.floor_id))
            .map(|(a, b)| a == b)
            .unwrap_or(true);
        if !emphasized {
            continue;
        }
        let elev = floor.map(|f| f.elevation_meters).unwrap_or(0.0);
        let [ax, az] = connection.from_local_position_xz;
        let [bx, bz] = connection.to_local_position_xz;
        let a = local_to_render(&transform, Vec3::new(ax, elev, az));
        let b = local_to_render(&transform, Vec3::new(bx, elev, bz));
        let line_color = if connection.enabled {
            Color::srgba(0.2, 0.75, 1.0, 0.95)
        } else {
            Color::srgba(0.5, 0.5, 0.5, 0.6)
        };
        gizmos.line(
            xz_to_render_y(a, FLOOR_Y_OFFSET + 0.14),
            xz_to_render_y(b, FLOOR_Y_OFFSET + 0.14),
            line_color,
        );
        gizmos.sphere(
            xz_to_render_y(a, FLOOR_Y_OFFSET + 0.14),
            connection.radius_meters * transform.scale.x,
            Color::srgba(0.2, 0.75, 1.0, 0.35),
        );
        gizmos.sphere(
            xz_to_render_y(b, FLOOR_Y_OFFSET + 0.14),
            connection.radius_meters * transform.scale.x,
            Color::srgba(0.2, 0.75, 1.0, 0.35),
        );
    }

    if inspection.editing || inspection.working_copy.is_some() {
        draw_edit_selection(&mut gizmos, &transform, blueprint, &inspection);
    }

    if inspection.draft_preview_active {
        if let Some(draft) = inspection.generated_draft.as_ref() {
            if !draft.adopted {
                draw_generated_draft_overlay(
                    &mut gizmos,
                    &transform,
                    &draft.blueprint,
                    selected_floor,
                );
            }
        }
    }
}

fn resolve_overlay_floor_id(
    blueprint: &BuildingNavigationBlueprint,
    inspection: &BlueprintInspectionState,
    overlay_focus: &InspectorOverlayFocus,
) -> Option<i32> {
    let candidate = inspection
        .selected_floor_id
        .or(overlay_focus.blueprint_floor_id);
    if let Some(id) = candidate {
        if blueprint.floors.iter().any(|floor| floor.floor_id == id) {
            return Some(id);
        }
    }
    blueprint.floors.first().map(|floor| floor.floor_id)
}

fn draw_edit_selection(
    gizmos: &mut Gizmos,
    transform: &Transform,
    blueprint: &BuildingNavigationBlueprint,
    inspection: &BlueprintInspectionState,
) {
    let selected_floor = inspection.selected_floor_id;
    match &inspection.selection {
        BlueprintEditSelection::Region {
            floor_id,
            region_key,
        } => {
            if selected_floor == Some(*floor_id) {
                if let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == *floor_id) {
                    if let Some(region) = floor.region_by_key(region_key) {
                        draw_region_outline(
                            gizmos,
                            transform,
                            floor.elevation_meters,
                            &region.walkable_outline.vertices_xz,
                            Color::srgba(1.0, 0.85, 0.2, 1.0),
                            0.12,
                        );
                    }
                }
            }
        }
        BlueprintEditSelection::Vertex {
            floor_id,
            region_key,
            index,
        } => {
            if selected_floor == Some(*floor_id) {
                if let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == *floor_id) {
                    if let Some(region) = floor.region_by_key(region_key) {
                        if let Some(&[x, z]) = region.walkable_outline.vertices_xz.get(*index) {
                            let pos =
                                local_to_render(transform, Vec3::new(x, floor.elevation_meters, z));
                            gizmos.sphere(
                                xz_to_render_y(pos, FLOOR_Y_OFFSET + 0.1),
                                0.22,
                                Color::srgba(1.0, 0.35, 0.1, 1.0),
                            );
                        }
                    }
                }
            }
        }
        BlueprintEditSelection::Edge {
            floor_id,
            region_key,
            index,
        } => {
            if selected_floor == Some(*floor_id) {
                if let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == *floor_id) {
                    if let Some(region) = floor.region_by_key(region_key) {
                        let verts = &region.walkable_outline.vertices_xz;
                        if let (Some(&[ax, az]), Some(&[bx, bz])) =
                            (verts.get(*index), verts.get((*index + 1) % verts.len()))
                        {
                            let a = local_to_render(
                                transform,
                                Vec3::new(ax, floor.elevation_meters, az),
                            );
                            let b = local_to_render(
                                transform,
                                Vec3::new(bx, floor.elevation_meters, bz),
                            );
                            gizmos.line(
                                xz_to_render_y(a, FLOOR_Y_OFFSET + 0.1),
                                xz_to_render_y(b, FLOOR_Y_OFFSET + 0.1),
                                Color::srgba(1.0, 0.55, 0.1, 1.0),
                            );
                        }
                    }
                }
            }
        }
        BlueprintEditSelection::Entrance { key } => {
            if let Some(entrance) = blueprint.entrances.iter().find(|e| e.key == *key) {
                let elev = floor_elevation(blueprint, &entrance.floor_key);
                let [x, z] = entrance.local_position_xz;
                let center = local_to_render(transform, Vec3::new(x, elev, z));
                gizmos.sphere(
                    xz_to_render_y(center, FLOOR_Y_OFFSET + 0.12),
                    entrance.radius_meters * transform.scale.x,
                    Color::srgba(1.0, 0.85, 0.15, 0.35),
                );
            }
        }
        BlueprintEditSelection::Transition { key }
        | BlueprintEditSelection::TransitionTo { key } => {
            if let Some(transition) = blueprint
                .vertical_transitions
                .iter()
                .find(|t| t.key == *key)
            {
                let from_elev = floor_elevation(blueprint, &transition.from_floor_key);
                let [fx, fz] = transition.from_local_position_xz;
                let from = local_to_render(transform, Vec3::new(fx, from_elev, fz));
                let to = local_to_render(transform, Vec3::from_array(transition.to_local_position));
                gizmos.line(
                    xz_to_render_y(from, FLOOR_Y_OFFSET + 0.12),
                    xz_to_render_y(to, FLOOR_Y_OFFSET + 0.12),
                    Color::srgba(0.85, 0.35, 1.0, 0.95),
                );
            }
        }
        BlueprintEditSelection::Connection { key }
        | BlueprintEditSelection::ConnectionFrom { key }
        | BlueprintEditSelection::ConnectionTo { key } => {
            if let Some(connection) = blueprint.region_connections.iter().find(|c| c.key == *key) {
                let elev = floor_elevation(blueprint, &connection.floor_key);
                let from = local_to_render(
                    transform,
                    Vec3::new(
                        connection.from_local_position_xz[0],
                        elev,
                        connection.from_local_position_xz[1],
                    ),
                );
                let to = local_to_render(
                    transform,
                    Vec3::new(
                        connection.to_local_position_xz[0],
                        elev,
                        connection.to_local_position_xz[1],
                    ),
                );
                gizmos.line(
                    xz_to_render_y(from, FLOOR_Y_OFFSET + 0.14),
                    xz_to_render_y(to, FLOOR_Y_OFFSET + 0.14),
                    Color::srgba(0.2, 0.95, 0.95, 0.95),
                );
            }
        }
        BlueprintEditSelection::None => {}
    }
}

fn floor_elevation(blueprint: &BuildingNavigationBlueprint, floor_key: &str) -> f32 {
    blueprint
        .floors
        .iter()
        .find(|f| f.key == floor_key)
        .map(|f| f.elevation_meters)
        .unwrap_or(0.0)
}

fn local_to_render(transform: &Transform, local: Vec3) -> Vec3 {
    blueprint_local_to_world(transform, Vec2::new(local.x, local.z), local.y)
}

fn draw_building_origin(gizmos: &mut Gizmos, transform: &Transform) {
    let origin = transform.translation;
    let axis_len = 1.5 * transform.scale.x.max(0.5);
    gizmos.line(
        origin,
        origin + transform.rotation * Vec3::X * axis_len,
        Color::srgba(0.9, 0.2, 0.2, 0.85),
    );
    gizmos.line(
        origin,
        origin + transform.rotation * Vec3::Z * axis_len,
        Color::srgba(0.2, 0.4, 0.95, 0.85),
    );
}

fn draw_generated_draft_overlay(
    gizmos: &mut Gizmos,
    transform: &Transform,
    blueprint: &BuildingNavigationBlueprint,
    selected_floor: Option<i32>,
) {
    let draft_edge = Color::srgba(1.0, 0.35, 0.95, 0.95);
    let draft_vertex = Color::srgba(1.0, 0.55, 0.85, 0.95);
    let draft_connection = Color::srgba(1.0, 0.55, 0.2, 0.9);
    let draft_entrance = Color::srgba(1.0, 0.65, 0.2, 0.9);
    let draft_transition = Color::srgba(0.85, 0.45, 1.0, 0.9);
    for floor in &blueprint.floors {
        let emphasized = selected_floor
            .map(|id| id == floor.floor_id)
            .unwrap_or(true);
        if !emphasized {
            continue;
        }
        for region in &floor.regions {
            let verts: Vec<Vec3> = region
                .walkable_outline
                .vertices_xz
                .iter()
                .map(|&[x, z]| local_to_render(transform, Vec3::new(x, floor.elevation_meters, z)))
                .map(|p| xz_to_render_y(p, FLOOR_Y_OFFSET + 0.12))
                .collect();
            for (i, j) in closed_polygon_boundary_segments(verts.len()) {
                gizmos.line(verts[i], verts[j], draft_edge);
            }
            for pos in &verts {
                gizmos.sphere(*pos, VERTEX_GIZMO_RADIUS * 0.9, draft_vertex);
            }
        }
    }
    for entrance in &blueprint.entrances {
        let floor_id = blueprint
            .floors
            .iter()
            .find(|f| f.key == entrance.floor_key)
            .map(|f| f.floor_id);
        let emphasized = selected_floor
            .zip(floor_id)
            .map(|(a, b)| a == b)
            .unwrap_or(true);
        if !emphasized {
            continue;
        }
        let elev = floor_elevation(blueprint, &entrance.floor_key);
        let [x, z] = entrance.local_position_xz;
        let center = xz_to_render_y(
            local_to_render(transform, Vec3::new(x, elev, z)),
            FLOOR_Y_OFFSET + 0.14,
        );
        gizmos.circle(
            Isometry3d::new(center, Quat::IDENTITY),
            entrance.radius_meters * transform.scale.x,
            draft_entrance,
        );
    }
    for transition in &blueprint.vertical_transitions {
        let from_floor = blueprint
            .floors
            .iter()
            .find(|f| f.key == transition.from_floor_key);
        let emphasized = selected_floor
            .zip(from_floor.map(|f| f.floor_id))
            .map(|(a, b)| a == b)
            .unwrap_or(true);
        if !emphasized {
            continue;
        }
        let from_elev = floor_elevation(blueprint, &transition.from_floor_key);
        let [fx, fz] = transition.from_local_position_xz;
        let from = xz_to_render_y(
            local_to_render(transform, Vec3::new(fx, from_elev, fz)),
            FLOOR_Y_OFFSET + 0.14,
        );
        let to = xz_to_render_y(
            local_to_render(transform, Vec3::from_array(transition.to_local_position)),
            FLOOR_Y_OFFSET + 0.14,
        );
        gizmos.line(from, to, draft_transition);
    }
    for connection in &blueprint.region_connections {
        let floor = blueprint
            .floors
            .iter()
            .find(|f| f.key == connection.floor_key);
        let emphasized = selected_floor
            .zip(floor.map(|f| f.floor_id))
            .map(|(a, b)| a == b)
            .unwrap_or(true);
        if !emphasized {
            continue;
        }
        let elev = floor.map(|f| f.elevation_meters).unwrap_or(0.0);
        let [ax, az] = connection.from_local_position_xz;
        let [bx, bz] = connection.to_local_position_xz;
        let a = local_to_render(&transform, Vec3::new(ax, elev, az));
        let b = local_to_render(&transform, Vec3::new(bx, elev, bz));
        gizmos.line(
            xz_to_render_y(a, FLOOR_Y_OFFSET + 0.2),
            xz_to_render_y(b, FLOOR_Y_OFFSET + 0.2),
            draft_connection,
        );
    }
}

fn draw_region_outline(
    gizmos: &mut Gizmos,
    transform: &Transform,
    elevation_meters: f32,
    vertices_xz: &[[f32; 2]],
    edge_color: Color,
    y_offset: f32,
) {
    let verts: Vec<Vec3> = vertices_xz
        .iter()
        .map(|&[x, z]| local_to_render(transform, Vec3::new(x, elevation_meters, z)))
        .map(|p| xz_to_render_y(p, FLOOR_Y_OFFSET + y_offset))
        .collect();
    for (i, j) in closed_polygon_boundary_segments(verts.len()) {
        gizmos.line(verts[i], verts[j], edge_color);
    }
}

fn draw_floor_polygon(
    gizmos: &mut Gizmos,
    transform: &Transform,
    floor: &NavigationFloorDefinition,
    alpha: f32,
    diagnostic: Option<&BlueprintDiagnosticFocus>,
    emphasized: bool,
) {
    for region in &floor.regions {
        let verts: Vec<Vec3> = region
            .walkable_outline
            .vertices_xz
            .iter()
            .map(|&[x, z]| local_to_render(transform, Vec3::new(x, floor.elevation_meters, z)))
            .map(|p| xz_to_render_y(p, FLOOR_Y_OFFSET))
            .collect();

        if verts.len() < 2 {
            continue;
        }

        let edge_color = Color::srgba(0.15, 1.0, 0.65, 0.9 * alpha);
        let vertex_color = Color::srgba(1.0, 1.0, 0.35, 0.95 * alpha);

        for (i, j) in closed_polygon_boundary_segments(verts.len()) {
            let highlight = diagnostic_is_edge(diagnostic, floor.floor_id, i);
            let color = if highlight {
                Color::srgba(1.0, 0.35, 0.15, 1.0)
            } else {
                edge_color
            };
            gizmos.line(verts[i], verts[j], color);
        }

        if emphasized {
            for (index, pos) in verts.iter().enumerate() {
                let highlight = diagnostic_is_vertex(diagnostic, floor.floor_id, index);
                let color = if highlight {
                    Color::srgba(1.0, 0.2, 0.2, 1.0)
                } else {
                    vertex_color
                };
                gizmos.sphere(*pos, VERTEX_GIZMO_RADIUS, color);
            }
        }
    }
}

fn draw_entrance(
    gizmos: &mut Gizmos,
    transform: &Transform,
    entrance: &NavigationEntranceDefinition,
    elevation: f32,
    diagnostic: Option<&BlueprintDiagnosticFocus>,
    blueprint: &BuildingNavigationBlueprint,
) {
    let [tx, tz] = entrance.local_position_xz;
    let threshold = local_to_render(transform, Vec3::new(tx, elevation, tz));
    let threshold_y = xz_to_render_y(threshold, FLOOR_Y_OFFSET + 0.04);

    let (tangent_xz, inward_xz, outward_xz, exterior) =
        entrance_glyph_axes(blueprint, entrance, elevation, transform, Vec2::new(tx, tz));

    let highlight = diagnostic
        .and_then(|d| d.entrance_key.as_deref())
        .map(|key| key == entrance.key)
        .unwrap_or(false);
    let stem_color = if highlight {
        Color::srgba(1.0, 0.55, 0.15, 1.0)
    } else {
        Color::srgba(0.95, 0.45, 0.12, 0.95)
    };
    let cap_color = if highlight {
        Color::srgba(1.0, 0.7, 0.25, 1.0)
    } else {
        Color::srgba(0.85, 0.55, 0.15, 0.9)
    };
    let scale = transform.scale.x;
    let bar_half = entrance.radius_meters * scale * 0.85;
    let cap_half = entrance.radius_meters * scale * 0.35;

    let tangent = transform.rotation * Vec3::new(tangent_xz.x, 0.0, tangent_xz.y);
    let inward = transform.rotation * Vec3::new(inward_xz.x, 0.0, inward_xz.y);
    let outward = transform.rotation * Vec3::new(outward_xz.x, 0.0, outward_xz.y);

    let bar_left = threshold_y - tangent * bar_half;
    let bar_right = threshold_y + tangent * bar_half;
    gizmos.line(bar_left, bar_right, stem_color);

    draw_cap_bar(gizmos, bar_left, inward, cap_half, cap_color);
    draw_cap_bar(gizmos, bar_right, outward, cap_half, cap_color);

    gizmos.sphere(threshold_y, 0.08 * scale, stem_color);

    let spawn = local_to_render(transform, Vec3::from_array(entrance.interior_spawn_local));
    let spawn_y = xz_to_render_y(spawn, FLOOR_Y_OFFSET + 0.06);
    gizmos.sphere(spawn_y, 0.1 * scale, Color::srgba(0.2, 0.85, 1.0, 0.85));
    gizmos.line(threshold_y, spawn_y, Color::srgba(0.2, 0.75, 1.0, 0.45));

    let exterior_render = local_to_render(transform, Vec3::new(exterior.x, 0.0, exterior.y));
    let exterior_y = xz_to_render_y(exterior_render, FLOOR_Y_OFFSET + 0.04);
    gizmos.sphere(
        exterior_y,
        0.08 * scale,
        Color::srgba(0.55, 0.85, 0.35, 0.85),
    );
}

fn entrance_glyph_axes(
    blueprint: &BuildingNavigationBlueprint,
    entrance: &NavigationEntranceDefinition,
    elevation: f32,
    transform: &Transform,
    threshold: Vec2,
) -> (Vec2, Vec2, Vec2, Vec2) {
    let floor = blueprint.floor_by_key(&entrance.floor_key);
    let region = floor.and_then(|floor| {
        blueprint
            .resolve_region_key(
                &entrance.floor_key,
                entrance.region_key.as_deref(),
                &entrance.key,
            )
            .ok()
            .and_then(|key| floor.region_by_key(key))
    });
    let projection = region.and_then(|region| {
        crate::world::nearest_boundary_projection_entrance(
            &region.walkable_outline.vertices_xz,
            threshold,
            f32::INFINITY,
            crate::world::ENTRANCE_CORNER_MARGIN,
        )
    });
    let tangent_xz = projection
        .map(|p| p.edge_tangent)
        .unwrap_or(Vec2::new(1.0, 0.0));
    let outward_xz = projection
        .map(|p| p.outward_normal)
        .unwrap_or(Vec2::new(0.0, -1.0));
    let inward_xz = -outward_xz;
    let exterior = region
        .and_then(|region| {
            crate::world::exterior_staging_for_entrance(
                entrance,
                region,
                crate::world::DEFAULT_EXTERIOR_STAGING_OFFSET,
            )
        })
        .unwrap_or(threshold);
    (
        tangent_xz.normalize_or_zero(),
        inward_xz.normalize_or_zero(),
        outward_xz.normalize_or_zero(),
        exterior,
    )
}

fn draw_cap_bar(gizmos: &mut Gizmos, center: Vec3, along: Vec3, half_width: f32, color: Color) {
    let along_flat = Vec3::new(along.x, 0.0, along.z);
    if along_flat.length_squared() <= 1e-8 {
        return;
    }
    let along_norm = along_flat.normalize();
    let tangent = Vec3::new(-along_norm.z, 0.0, along_norm.x);
    let offset = tangent * half_width;
    gizmos.line(center - offset, center + offset, color);
}

fn draw_vertical_transition(
    gizmos: &mut Gizmos,
    transform: &Transform,
    transition: &NavigationVerticalTransitionDefinition,
    blueprint: &BuildingNavigationBlueprint,
    diagnostic: Option<&BlueprintDiagnosticFocus>,
) {
    let from_elev = floor_elevation(blueprint, &transition.from_floor_key);
    let [fx, fz] = transition.from_local_position_xz;
    let from = local_to_render(transform, Vec3::new(fx, from_elev, fz));
    let from = xz_to_render_y(from, FLOOR_Y_OFFSET + 0.05);
    let to = local_to_render(transform, Vec3::from_array(transition.to_local_position));
    let to = xz_to_render_y(to, FLOOR_Y_OFFSET + 0.05);
    let highlight = diagnostic
        .and_then(|d| d.transition_key.as_deref())
        .map(|key| key == transition.key)
        .unwrap_or(false);
    let color = if highlight {
        Color::srgba(0.85, 0.35, 1.0, 1.0)
    } else {
        Color::srgba(0.55, 0.35, 0.95, 0.9)
    };
    let radius = transition.from_radius_meters * transform.scale.x;
    gizmos.circle(Isometry3d::new(from, Quat::IDENTITY), radius, color);
    gizmos.line(from, to, color);
    gizmos.sphere(to, 0.14, color);
}

fn diagnostic_is_vertex(
    diagnostic: Option<&BlueprintDiagnosticFocus>,
    floor_id: i32,
    index: usize,
) -> bool {
    diagnostic
        .and_then(|d| {
            if d.floor_id == Some(floor_id) && d.vertex_index == Some(index) {
                Some(true)
            } else {
                None
            }
        })
        .unwrap_or(false)
}

fn diagnostic_is_edge(
    diagnostic: Option<&BlueprintDiagnosticFocus>,
    floor_id: i32,
    edge_index: usize,
) -> bool {
    diagnostic
        .and_then(|d| {
            if d.floor_id == Some(floor_id) && d.edge_index == Some(edge_index) {
                Some(true)
            } else {
                None
            }
        })
        .unwrap_or(false)
}
