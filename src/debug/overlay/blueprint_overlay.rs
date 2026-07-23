//! Generated navigation blueprint overlay (NV1.2.5). Read-only blueprint geometry.

use bevy::prelude::*;

use crate::dev::{
    BlueprintEditSelection, BlueprintInspectionState, WorldInspectorState,
};
use crate::terrain::TerrainRenderAssets;
use crate::world::{
    BlueprintDiagnosticFocus, BuildingCatalog, BuildingNavigationBlueprint,
    BuildingNavigationBlueprintCatalog, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationVerticalTransitionDefinition, WorldConfig, WorldData, building_model_render_transform,
    resolve_building_navigation_blueprint,
};

use super::helpers::xz_to_render_y;
use crate::debug::settings::DebugOverlaySettings;
use crate::debug::InspectorOverlayFocus;

const FLOOR_Y_OFFSET: f32 = 0.08;
const OTHER_FLOOR_ALPHA: f32 = 0.22;

pub fn draw_blueprint_debug_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    building_catalog: Res<BuildingCatalog>,
    nav_catalog: Res<BuildingNavigationBlueprintCatalog>,
    settings: Res<DebugOverlaySettings>,
    inspection: Res<BlueprintInspectionState>,
    inspector: Res<WorldInspectorState>,
    overlay_focus: Res<InspectorOverlayFocus>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.blueprint_overlay_active() && !inspection.active {
        return;
    }

    let building_id = inspection
        .building_id
        .or(inspector.selected_building)
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

    let blueprint_owned = if inspection.editing {
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
    let blueprint = if inspection.editing {
        inspection.working_copy.as_ref().or(blueprint_owned.as_ref())
    } else {
        blueprint_owned.as_ref()
    };
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

    let selected_floor = inspection
        .selected_floor_id
        .or(overlay_focus.blueprint_floor_id);
    let diagnostic = overlay_focus
        .blueprint_diagnostic
        .as_ref()
        .or(inspection.focused_diagnostic_index.and_then(|index| {
            inspector
                .blueprint_snapshot
                .as_ref()
                .and_then(|snap| snap.validation.diagnostics.get(index))
                .and_then(|d| d.focus.as_ref())
        }));

    draw_building_origin(&mut gizmos, &transform);

    for floor in &blueprint.floors {
        let emphasized = selected_floor.map(|id| id == floor.floor_id).unwrap_or(true);
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
        draw_vertical_transition(
            &mut gizmos,
            &transform,
            transition,
            blueprint,
            diagnostic,
        );
    }

    if inspection.editing {
        draw_edit_selection(&mut gizmos, &transform, blueprint, &inspection);
    }
}

fn draw_edit_selection(
    gizmos: &mut Gizmos,
    transform: &Transform,
    blueprint: &BuildingNavigationBlueprint,
    inspection: &BlueprintInspectionState,
) {
    let selected_floor = inspection.selected_floor_id;
    match &inspection.selection {
        BlueprintEditSelection::Vertex { floor_id, index } => {
            if selected_floor == Some(*floor_id) {
                if let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == *floor_id) {
                    if let Some(&[x, z]) = floor.walkable_outline.vertices_xz.get(*index) {
                        let pos = local_to_render(
                            transform,
                            Vec3::new(x, floor.elevation_meters, z),
                        );
                        gizmos.sphere(
                            xz_to_render_y(pos, FLOOR_Y_OFFSET + 0.1),
                            0.22,
                            Color::srgba(1.0, 0.35, 0.1, 1.0),
                        );
                    }
                }
            }
        }
        BlueprintEditSelection::Edge { floor_id, index } => {
            if selected_floor == Some(*floor_id) {
                if let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == *floor_id) {
                    let verts = &floor.walkable_outline.vertices_xz;
                    if let (Some(&[ax, az]), Some(&[bx, bz])) = (
                        verts.get(*index),
                        verts.get((*index + 1) % verts.len()),
                    ) {
                        let a = local_to_render(transform, Vec3::new(ax, floor.elevation_meters, az));
                        let b = local_to_render(transform, Vec3::new(bx, floor.elevation_meters, bz));
                        gizmos.line(
                            xz_to_render_y(a, FLOOR_Y_OFFSET + 0.1),
                            xz_to_render_y(b, FLOOR_Y_OFFSET + 0.1),
                            Color::srgba(1.0, 0.55, 0.1, 1.0),
                        );
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
        BlueprintEditSelection::Transition { key } | BlueprintEditSelection::TransitionTo { key } => {
            if let Some(transition) = blueprint.vertical_transitions.iter().find(|t| t.key == *key) {
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
    transform.transform_point(local)
}

fn draw_building_origin(gizmos: &mut Gizmos, transform: &Transform) {
    let origin = transform.translation;
    let axis_len = 1.5 * transform.scale.x.max(0.5);
    gizmos.line(origin, origin + transform.rotation * Vec3::X * axis_len, Color::srgba(0.9, 0.2, 0.2, 0.85));
    gizmos.line(origin, origin + transform.rotation * Vec3::Z * axis_len, Color::srgba(0.2, 0.4, 0.95, 0.85));
}

fn draw_floor_polygon(
    gizmos: &mut Gizmos,
    transform: &Transform,
    floor: &NavigationFloorDefinition,
    alpha: f32,
    diagnostic: Option<&BlueprintDiagnosticFocus>,
    emphasized: bool,
) {
    let verts: Vec<Vec3> = floor
        .walkable_outline
        .vertices_xz
        .iter()
        .map(|&[x, z]| {
            local_to_render(
                transform,
                Vec3::new(x, floor.elevation_meters, z),
            )
        })
        .map(|p| xz_to_render_y(p, FLOOR_Y_OFFSET))
        .collect();

    if verts.len() < 2 {
        return;
    }

    let outline_color = Color::srgba(0.1, 0.95, 0.55, 0.35 * alpha);
    let edge_color = Color::srgba(0.15, 1.0, 0.65, 0.9 * alpha);
    let vertex_color = Color::srgba(1.0, 1.0, 0.35, 0.95 * alpha);

    for i in 0..verts.len() {
        let a = verts[i];
        let b = verts[(i + 1) % verts.len()];
        let highlight = diagnostic_is_edge(diagnostic, floor.floor_id, i);
        let color = if highlight {
            Color::srgba(1.0, 0.35, 0.15, 1.0)
        } else {
            edge_color
        };
        gizmos.line(a, b, color);
        if emphasized && i + 2 <= verts.len() {
            gizmos.line(a, verts[(i + 2) % verts.len()], outline_color);
        }
    }

    if emphasized {
        for (index, pos) in verts.iter().enumerate() {
            let highlight = diagnostic_is_vertex(diagnostic, floor.floor_id, index);
            let color = if highlight {
                Color::srgba(1.0, 0.2, 0.2, 1.0)
            } else {
                vertex_color
            };
            gizmos.sphere(*pos, 0.12, color);
        }
    }
}

fn draw_entrance(
    gizmos: &mut Gizmos,
    transform: &Transform,
    entrance: &NavigationEntranceDefinition,
    elevation: f32,
    diagnostic: Option<&BlueprintDiagnosticFocus>,
) {
    let [x, z] = entrance.local_position_xz;
    let center = local_to_render(transform, Vec3::new(x, elevation, z));
    let center = xz_to_render_y(center, FLOOR_Y_OFFSET + 0.04);
    let highlight = diagnostic
        .and_then(|d| d.entrance_key.as_deref())
        .map(|key| key == entrance.key)
        .unwrap_or(false);
    let color = if highlight {
        Color::srgba(1.0, 0.45, 0.1, 1.0)
    } else {
        Color::srgba(0.95, 0.75, 0.15, 0.95)
    };
    let radius = entrance.radius_meters * transform.scale.x;
    gizmos.circle(
        Isometry3d::new(center, Quat::IDENTITY),
        radius,
        color,
    );
    let forward = transform.rotation * Vec3::new(0.0, 0.0, -1.0);
    gizmos.line(center, center + forward * radius * 1.4, color);
    let spawn = local_to_render(
        transform,
        Vec3::from_array(entrance.interior_spawn_local),
    );
    let spawn = xz_to_render_y(spawn, FLOOR_Y_OFFSET + 0.06);
    gizmos.line(center, spawn, Color::srgba(0.95, 0.75, 0.15, 0.55));
    gizmos.sphere(spawn, 0.1, Color::srgba(0.95, 0.75, 0.15, 0.8));
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
    gizmos.circle(
        Isometry3d::new(from, Quat::IDENTITY),
        radius,
        color,
    );
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
