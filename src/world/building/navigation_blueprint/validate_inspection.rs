//! Extended blueprint validation for dev inspection (NV1.2.5).

use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationVerticalTransitionDefinition,
};
use super::id::BuildingNavigationBlueprintId;
use bevy::prelude::Vec2;

const DUPLICATE_VERTEX_EPSILON: f32 = 0.05;
const MIN_EDGE_LENGTH: f32 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlueprintDiagnosticLevel {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintDiagnosticFocus {
    pub floor_id: Option<i32>,
    pub vertex_index: Option<usize>,
    pub edge_index: Option<usize>,
    pub entrance_key: Option<String>,
    pub transition_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintValidationDiagnostic {
    pub level: BlueprintDiagnosticLevel,
    pub code: &'static str,
    pub message: String,
    pub focus: Option<BlueprintDiagnosticFocus>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BlueprintInspectionValidation {
    pub diagnostics: Vec<BlueprintValidationDiagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
}

impl BlueprintInspectionValidation {
    pub fn valid(&self) -> bool {
        self.error_count == 0
    }
}

pub fn validate_blueprint_for_inspection(
    blueprint: &BuildingNavigationBlueprint,
) -> BlueprintInspectionValidation {
    let mut diagnostics = Vec::new();

    if let Err(err) = blueprint.validate() {
        diagnostics.push(BlueprintValidationDiagnostic {
            level: BlueprintDiagnosticLevel::Error,
            code: "schema_invalid",
            message: err.to_string(),
            focus: None,
        });
    }

    let mut floor_keys = std::collections::BTreeSet::new();
    let mut floor_ids = std::collections::BTreeSet::new();
    for floor in &blueprint.floors {
        if !floor_keys.insert(floor.key.clone()) {
            push_error(
                &mut diagnostics,
                "duplicate_floor_key",
                format!("duplicate floor key `{}`", floor.key),
                BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    ..Default::default()
                },
            );
        }
        if !floor_ids.insert(floor.floor_id) {
            push_error(
                &mut diagnostics,
                "duplicate_floor_id",
                format!("duplicate floor id {}", floor.floor_id),
                BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    ..Default::default()
                },
            );
        }
        validate_floor_polygon(floor, &mut diagnostics);
    }

    let mut feature_keys = std::collections::BTreeSet::new();
    for entrance in &blueprint.entrances {
        if !feature_keys.insert(entrance.key.clone()) {
            push_error(
                &mut diagnostics,
                "duplicate_entrance_key",
                format!("duplicate entrance key `{}`", entrance.key),
                BlueprintDiagnosticFocus {
                    entrance_key: Some(entrance.key.clone()),
                    ..Default::default()
                },
            );
        }
        validate_entrance(entrance, blueprint, &mut diagnostics);
    }
    for transition in &blueprint.vertical_transitions {
        if !feature_keys.insert(transition.key.clone()) {
            push_error(
                &mut diagnostics,
                "duplicate_transition_key",
                format!("duplicate transition key `{}`", transition.key),
                BlueprintDiagnosticFocus {
                    transition_key: Some(transition.key.clone()),
                    ..Default::default()
                },
            );
        }
        validate_transition(transition, blueprint, &mut diagnostics);
    }

    if blueprint.floors.is_empty() {
        push_error(
            &mut diagnostics,
            "no_floors",
            "blueprint has no floors".into(),
            BlueprintDiagnosticFocus::default(),
        );
    }
    if blueprint.entrances.is_empty() {
        push_warning(
            &mut diagnostics,
            "no_entrances",
            "blueprint has no entrances".into(),
            None,
        );
    }

    diagnostics.push(BlueprintValidationDiagnostic {
        level: BlueprintDiagnosticLevel::Info,
        code: "generator_revision",
        message: format!(
            "blueprint `{}` schema={} generator={:?}",
            blueprint.id,
            blueprint.schema_version,
            blueprint.metadata.generation_revision
        ),
        focus: None,
    });

    summarize(diagnostics)
}

impl Default for BlueprintDiagnosticFocus {
    fn default() -> Self {
        Self {
            floor_id: None,
            vertex_index: None,
            edge_index: None,
            entrance_key: None,
            transition_key: None,
        }
    }
}

fn validate_floor_polygon(
    floor: &NavigationFloorDefinition,
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
) {
    let focus_floor = BlueprintDiagnosticFocus {
        floor_id: Some(floor.floor_id),
        ..Default::default()
    };
    let verts = &floor.walkable_outline.vertices_xz;
    if verts.len() < 3 {
        push_error(
            diagnostics,
            "polygon_too_few_vertices",
            format!("floor {} has fewer than three vertices", floor.floor_id),
            focus_floor.clone(),
        );
        return;
    }

    let mut unique: Vec<(Vec2, usize)> = Vec::new();
    for (index, [x, z]) in verts.iter().enumerate() {
        if !x.is_finite() || !z.is_finite() {
            push_error(
                diagnostics,
                "non_finite_vertex",
                format!("floor {} vertex {index} is non-finite", floor.floor_id),
                BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    vertex_index: Some(index),
                    ..Default::default()
                },
            );
        }
        let p = Vec2::new(*x, *z);
        if unique
            .iter()
            .any(|(q, _)| q.distance(p) < DUPLICATE_VERTEX_EPSILON)
        {
            push_warning(
                diagnostics,
                "duplicate_vertex",
                format!("floor {} vertex {index} duplicates an earlier vertex", floor.floor_id),
                Some(BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    vertex_index: Some(index),
                    ..Default::default()
                }),
            );
        } else {
            unique.push((p, index));
        }
    }

    if unique.len() < 3 {
        push_error(
            diagnostics,
            "degenerate_polygon",
            format!("floor {} has fewer than three unique vertices", floor.floor_id),
            focus_floor.clone(),
        );
        return;
    }

    let area = floor.walkable_outline.signed_area();
    if area.abs() <= f32::EPSILON {
        push_error(
            diagnostics,
            "zero_area_polygon",
            format!("floor {} polygon has zero area", floor.floor_id),
            focus_floor.clone(),
        );
    } else if area < 0.0 {
        push_warning(
            diagnostics,
            "clockwise_winding",
            format!(
                "floor {} polygon appears clockwise (negative signed area)",
                floor.floor_id
            ),
            Some(focus_floor.clone()),
        );
    }

    for i in 0..verts.len() {
        let a = Vec2::new(verts[i][0], verts[i][1]);
        let b = Vec2::new(verts[(i + 1) % verts.len()][0], verts[(i + 1) % verts.len()][1]);
        if a.distance(b) < MIN_EDGE_LENGTH {
            push_warning(
                diagnostics,
                "short_edge",
                format!("floor {} edge {i} is very short", floor.floor_id),
                Some(BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    edge_index: Some(i),
                    ..Default::default()
                }),
            );
        }
    }

    if polygon_self_intersects(verts) {
        push_error(
            diagnostics,
            "self_intersection",
            format!("floor {} polygon self-intersects", floor.floor_id),
            focus_floor,
        );
    }
}

fn validate_entrance(
    entrance: &NavigationEntranceDefinition,
    blueprint: &BuildingNavigationBlueprint,
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
) {
    let focus = BlueprintDiagnosticFocus {
        entrance_key: Some(entrance.key.clone()),
        ..Default::default()
    };
    if entrance.radius_meters <= 0.0 || !entrance.radius_meters.is_finite() {
        push_error(
            diagnostics,
            "invalid_entrance_radius",
            format!("entrance `{}` has invalid radius", entrance.key),
            focus.clone(),
        );
    }
    let Some(floor) = blueprint.floor_by_key(&entrance.floor_key) else {
        push_error(
            diagnostics,
            "entrance_floor_missing",
            format!(
                "entrance `{}` references missing floor `{}`",
                entrance.key, entrance.floor_key
            ),
            focus,
        );
        return;
    };
    let pos = Vec2::new(entrance.local_position_xz[0], entrance.local_position_xz[1]);
    if !point_on_polygon_boundary(&floor.walkable_outline.vertices_xz, pos, entrance.radius_meters)
    {
        push_warning(
            diagnostics,
            "entrance_off_boundary",
            format!(
                "entrance `{}` is not near floor {} outline boundary",
                entrance.key, floor.floor_id
            ),
            Some(BlueprintDiagnosticFocus {
                floor_id: Some(floor.floor_id),
                entrance_key: Some(entrance.key.clone()),
                ..Default::default()
            }),
        );
    }
}

fn validate_transition(
    transition: &NavigationVerticalTransitionDefinition,
    blueprint: &BuildingNavigationBlueprint,
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
) {
    let focus = BlueprintDiagnosticFocus {
        transition_key: Some(transition.key.clone()),
        ..Default::default()
    };
    let from = blueprint.floor_by_key(&transition.from_floor_key);
    let to = blueprint.floor_by_key(&transition.to_floor_key);
    if from.is_none() {
        push_error(
            diagnostics,
            "transition_from_missing",
            format!(
                "transition `{}` from floor `{}` missing",
                transition.key, transition.from_floor_key
            ),
            focus.clone(),
        );
    }
    if to.is_none() {
        push_error(
            diagnostics,
            "transition_to_missing",
            format!(
                "transition `{}` to floor `{}` missing",
                transition.key, transition.to_floor_key
            ),
            focus.clone(),
        );
    }
    if let (Some(from), Some(to)) = (from, to) {
        let from_pos = Vec2::new(
            transition.from_local_position_xz[0],
            transition.from_local_position_xz[1],
        );
        if !point_inside_polygon(&from.walkable_outline.vertices_xz, from_pos) {
            push_warning(
                diagnostics,
                "transition_outside_from_floor",
                format!(
                    "transition `{}` start lies outside floor {} polygon",
                    transition.key, from.floor_id
                ),
                Some(BlueprintDiagnosticFocus {
                    floor_id: Some(from.floor_id),
                    transition_key: Some(transition.key.clone()),
                    ..Default::default()
                }),
            );
        }
        let to_pos = Vec2::new(transition.to_local_position[0], transition.to_local_position[2]);
        if !point_inside_polygon(&to.walkable_outline.vertices_xz, to_pos) {
            push_warning(
                diagnostics,
                "transition_outside_to_floor",
                format!(
                    "transition `{}` destination lies outside floor {} polygon",
                    transition.key, to.floor_id
                ),
                Some(BlueprintDiagnosticFocus {
                    floor_id: Some(to.floor_id),
                    transition_key: Some(transition.key.clone()),
                    ..Default::default()
                }),
            );
        }
    }
}

fn polygon_self_intersects(vertices: &[[f32; 2]]) -> bool {
    let n = vertices.len();
    if n < 4 {
        return false;
    }
    let edges: Vec<(Vec2, Vec2)> = (0..n)
        .map(|i| {
            let a = Vec2::new(vertices[i][0], vertices[i][1]);
            let b = Vec2::new(vertices[(i + 1) % n][0], vertices[(i + 1) % n][1]);
            (a, b)
        })
        .collect();
    for i in 0..edges.len() {
        for j in (i + 1)..edges.len() {
            if j == i + 1 || (i == 0 && j + 1 == edges.len()) {
                continue;
            }
            if segments_intersect(edges[i].0, edges[i].1, edges[j].0, edges[j].1) {
                return true;
            }
        }
    }
    false
}

fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    fn orient(a: Vec2, b: Vec2, c: Vec2) -> f32 {
        (b - a).perp_dot(c - a)
    }
    let o1 = orient(a1, a2, b1);
    let o2 = orient(a1, a2, b2);
    let o3 = orient(b1, b2, a1);
    let o4 = orient(b1, b2, a2);
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

fn point_inside_polygon(vertices: &[[f32; 2]], point: Vec2) -> bool {
    let mut inside = false;
    let mut j = vertices.len().wrapping_sub(1);
    for (i, [x, z]) in vertices.iter().enumerate() {
        let pi = Vec2::new(*x, *z);
        let pj = Vec2::new(vertices[j][0], vertices[j][1]);
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y + f32::EPSILON) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn point_on_polygon_boundary(vertices: &[[f32; 2]], point: Vec2, tolerance: f32) -> bool {
    let n = vertices.len();
    if n < 2 {
        return false;
    }
    for i in 0..n {
        let a = Vec2::new(vertices[i][0], vertices[i][1]);
        let b = Vec2::new(vertices[(i + 1) % n][0], vertices[(i + 1) % n][1]);
        if distance_point_to_segment(point, a, b) <= tolerance {
            return true;
        }
    }
    false
}

fn distance_point_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn push_error(
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
    code: &'static str,
    message: String,
    focus: BlueprintDiagnosticFocus,
) {
    diagnostics.push(BlueprintValidationDiagnostic {
        level: BlueprintDiagnosticLevel::Error,
        code,
        message,
        focus: Some(focus),
    });
}

fn push_warning(
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
    code: &'static str,
    message: String,
    focus: Option<BlueprintDiagnosticFocus>,
) {
    diagnostics.push(BlueprintValidationDiagnostic {
        level: BlueprintDiagnosticLevel::Warning,
        code,
        message,
        focus,
    });
}

fn summarize(diagnostics: Vec<BlueprintValidationDiagnostic>) -> BlueprintInspectionValidation {
    let error_count = diagnostics
        .iter()
        .filter(|d| d.level == BlueprintDiagnosticLevel::Error)
        .count();
    let warning_count = diagnostics
        .iter()
        .filter(|d| d.level == BlueprintDiagnosticLevel::Warning)
        .count();
    let info_count = diagnostics
        .iter()
        .filter(|d| d.level == BlueprintDiagnosticLevel::Info)
        .count();
    BlueprintInspectionValidation {
        diagnostics,
        error_count,
        warning_count,
        info_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::definition::{
        BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
        NavigationPolygon2d,
    };

    #[test]
    fn self_intersecting_polygon_reports_error() {
        let blueprint = BuildingNavigationBlueprint::new("bad", "Bad")
            .with_floors(vec![NavigationFloorDefinition {
                floor_id: 0,
                key: "floor_0".to_string(),
                display_label: "F0".to_string(),
                elevation_meters: 0.0,
                visibility_group_id: 1,
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![[0.0, 0.0], [4.0, 4.0], [4.0, 0.0], [0.0, 4.0]],
                },
            }])
            .with_entrances(vec![NavigationEntranceDefinition {
                key: "door".to_string(),
                floor_key: "floor_0".to_string(),
                local_position_xz: [2.0, 0.0],
                radius_meters: 1.0,
                interior_spawn_local: [2.0, 0.0, 1.0],
                bidirectional: true,
            }]);
        let report = validate_blueprint_for_inspection(&blueprint);
        assert!(!report.valid());
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == "self_intersection"));
    }
}
