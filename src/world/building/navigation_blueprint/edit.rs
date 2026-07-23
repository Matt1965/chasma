//! Dev blueprint edit operations (NV1.4). Pure data mutations with lightweight guards.

use bevy::prelude::Vec2;

use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationPolygon2d,
    NavigationVerticalTransitionDefinition, NavigationVerticalTransitionKind,
};
use super::validate_inspection::validate_blueprint_for_inspection;

const MIN_VERTEX_COUNT: usize = 3;
const MIN_EDGE_LENGTH_SQ: f32 = 0.1 * 0.1;

#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintEditOutcome {
    pub applied: bool,
    pub message: Option<String>,
}

impl BlueprintEditOutcome {
    fn ok() -> Self {
        Self {
            applied: true,
            message: None,
        }
    }

    fn rejected(message: impl Into<String>) -> Self {
        Self {
            applied: false,
            message: Some(message.into()),
        }
    }
}

pub fn move_floor_vertex(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    vertex_index: usize,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint.floors.iter_mut().find(|floor| floor.floor_id == floor_id) else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    let Some(vertex) = floor.walkable_outline.vertices_xz.get_mut(vertex_index) else {
        return BlueprintEditOutcome::rejected("vertex not found");
    };
    if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
        return BlueprintEditOutcome::rejected("vertex position must be finite");
    }
    *vertex = local_xz;
    if let Some(message) = polygon_edit_error(floor) {
        return BlueprintEditOutcome::rejected(message);
    }
    BlueprintEditOutcome::ok()
}

pub fn insert_vertex_on_edge(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    edge_index: usize,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint.floors.iter_mut().find(|floor| floor.floor_id == floor_id) else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    let vertex_count = floor.walkable_outline.vertices_xz.len();
    if vertex_count < MIN_VERTEX_COUNT || edge_index >= vertex_count {
        return BlueprintEditOutcome::rejected("invalid edge");
    }
    if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
        return BlueprintEditOutcome::rejected("vertex position must be finite");
    }
    floor
        .walkable_outline
        .vertices_xz
        .insert(edge_index + 1, local_xz);
    if let Some(message) = polygon_edit_error(floor) {
        floor.walkable_outline.vertices_xz.remove(edge_index + 1);
        return BlueprintEditOutcome::rejected(message);
    }
    BlueprintEditOutcome::ok()
}

pub fn delete_floor_vertex(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    vertex_index: usize,
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint.floors.iter_mut().find(|floor| floor.floor_id == floor_id) else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    if floor.walkable_outline.vertices_xz.len() <= MIN_VERTEX_COUNT {
        return BlueprintEditOutcome::rejected("floor polygon must keep at least three vertices");
    }
    if vertex_index >= floor.walkable_outline.vertices_xz.len() {
        return BlueprintEditOutcome::rejected("vertex not found");
    }
    floor.walkable_outline.vertices_xz.remove(vertex_index);
    if let Some(message) = polygon_edit_error(floor) {
        return BlueprintEditOutcome::rejected(message);
    }
    BlueprintEditOutcome::ok()
}

pub fn move_entrance(
    blueprint: &mut BuildingNavigationBlueprint,
    entrance_key: &str,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    let Some(entrance) = blueprint
        .entrances
        .iter_mut()
        .find(|entrance| entrance.key == entrance_key)
    else {
        return BlueprintEditOutcome::rejected("entrance not found");
    };
    if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
        return BlueprintEditOutcome::rejected("entrance position must be finite");
    }
    let delta = Vec2::new(
        local_xz[0] - entrance.local_position_xz[0],
        local_xz[1] - entrance.local_position_xz[1],
    );
    entrance.local_position_xz = local_xz;
    entrance.interior_spawn_local[0] += delta.x;
    entrance.interior_spawn_local[2] += delta.y;
    BlueprintEditOutcome::ok()
}

pub fn set_entrance_radius(
    blueprint: &mut BuildingNavigationBlueprint,
    entrance_key: &str,
    radius_meters: f32,
) -> BlueprintEditOutcome {
    if !(radius_meters > 0.0) || !radius_meters.is_finite() {
        return BlueprintEditOutcome::rejected("entrance radius must be positive and finite");
    }
    let Some(entrance) = blueprint
        .entrances
        .iter_mut()
        .find(|entrance| entrance.key == entrance_key)
    else {
        return BlueprintEditOutcome::rejected("entrance not found");
    };
    entrance.radius_meters = radius_meters;
    BlueprintEditOutcome::ok()
}

pub fn add_entrance_on_floor(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_key: &str,
    local_xz: [f32; 2],
    radius_meters: f32,
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint.floor_by_key(floor_key) else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    if !(radius_meters > 0.0) || !radius_meters.is_finite() {
        return BlueprintEditOutcome::rejected("entrance radius must be positive and finite");
    }
    if !point_in_polygon_local(&floor.walkable_outline, local_xz) {
        return BlueprintEditOutcome::rejected("entrance must lie on the selected floor outline");
    }
    let key = next_feature_key(
        blueprint
            .entrances
            .iter()
            .map(|entrance| entrance.key.as_str())
            .chain(
                blueprint
                    .vertical_transitions
                    .iter()
                    .map(|transition| transition.key.as_str()),
            ),
        "entrance",
    );
    blueprint.entrances.push(NavigationEntranceDefinition {
        key,
        floor_key: floor_key.to_string(),
        local_position_xz: local_xz,
        radius_meters,
        interior_spawn_local: [local_xz[0], floor.elevation_meters, local_xz[1] + 1.0],
        bidirectional: true,
    });
    BlueprintEditOutcome::ok()
}

pub fn delete_entrance(
    blueprint: &mut BuildingNavigationBlueprint,
    entrance_key: &str,
) -> BlueprintEditOutcome {
    let before = blueprint.entrances.len();
    blueprint.entrances.retain(|entrance| entrance.key != entrance_key);
    if blueprint.entrances.len() == before {
        return BlueprintEditOutcome::rejected("entrance not found");
    }
    BlueprintEditOutcome::ok()
}

pub fn move_transition_from(
    blueprint: &mut BuildingNavigationBlueprint,
    transition_key: &str,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    let Some(transition) = blueprint
        .vertical_transitions
        .iter_mut()
        .find(|transition| transition.key == transition_key)
    else {
        return BlueprintEditOutcome::rejected("transition not found");
    };
    if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
        return BlueprintEditOutcome::rejected("transition position must be finite");
    }
    transition.from_local_position_xz = local_xz;
    BlueprintEditOutcome::ok()
}

pub fn move_transition_to(
    blueprint: &mut BuildingNavigationBlueprint,
    transition_key: &str,
    local_position: [f32; 3],
) -> BlueprintEditOutcome {
    let Some(transition) = blueprint
        .vertical_transitions
        .iter_mut()
        .find(|transition| transition.key == transition_key)
    else {
        return BlueprintEditOutcome::rejected("transition not found");
    };
    if local_position.iter().any(|value| !value.is_finite()) {
        return BlueprintEditOutcome::rejected("transition position must be finite");
    }
    transition.to_local_position = local_position;
    BlueprintEditOutcome::ok()
}

pub fn set_transition_radius(
    blueprint: &mut BuildingNavigationBlueprint,
    transition_key: &str,
    radius_meters: f32,
) -> BlueprintEditOutcome {
    if !(radius_meters > 0.0) || !radius_meters.is_finite() {
        return BlueprintEditOutcome::rejected("transition radius must be positive and finite");
    }
    let Some(transition) = blueprint
        .vertical_transitions
        .iter_mut()
        .find(|transition| transition.key == transition_key)
    else {
        return BlueprintEditOutcome::rejected("transition not found");
    };
    transition.from_radius_meters = radius_meters;
    BlueprintEditOutcome::ok()
}

pub fn add_stair_transition(
    blueprint: &mut BuildingNavigationBlueprint,
    from_floor_key: &str,
    to_floor_key: &str,
    from_local_xz: [f32; 2],
    to_local_position: [f32; 3],
    radius_meters: f32,
) -> BlueprintEditOutcome {
    if blueprint.floor_by_key(from_floor_key).is_none()
        || blueprint.floor_by_key(to_floor_key).is_none()
    {
        return BlueprintEditOutcome::rejected("transition floor not found");
    }
    let key = next_feature_key(
        blueprint
            .entrances
            .iter()
            .map(|entrance| entrance.key.as_str())
            .chain(
                blueprint
                    .vertical_transitions
                    .iter()
                    .map(|transition| transition.key.as_str()),
            ),
        "stairs",
    );
    blueprint
        .vertical_transitions
        .push(NavigationVerticalTransitionDefinition {
            key,
            kind: NavigationVerticalTransitionKind::Stair,
            from_floor_key: from_floor_key.to_string(),
            to_floor_key: to_floor_key.to_string(),
            from_local_position_xz: from_local_xz,
            from_radius_meters: radius_meters,
            to_local_position,
            bidirectional: true,
        });
    BlueprintEditOutcome::ok()
}

pub fn delete_transition(
    blueprint: &mut BuildingNavigationBlueprint,
    transition_key: &str,
) -> BlueprintEditOutcome {
    let before = blueprint.vertical_transitions.len();
    blueprint
        .vertical_transitions
        .retain(|transition| transition.key != transition_key);
    if blueprint.vertical_transitions.len() == before {
        return BlueprintEditOutcome::rejected("transition not found");
    }
    BlueprintEditOutcome::ok()
}

pub fn prepare_blueprint_for_save(
    mut blueprint: BuildingNavigationBlueprint,
) -> Result<BuildingNavigationBlueprint, String> {
    blueprint.validate().map_err(|err| err.to_string())?;
    let validation = validate_blueprint_for_inspection(&blueprint);
    if !validation.valid() {
        return Err(format!(
            "blueprint has {} validation error(s); fix before saving",
            validation.error_count
        ));
    }
    blueprint
        .metadata
        .extensions
        .insert("edited_by".to_string(), "dev_editor".to_string());
    blueprint.metadata.generation_revision = Some(
        blueprint
            .metadata
            .generation_revision
            .unwrap_or(0)
            .saturating_add(1),
    );
    Ok(blueprint)
}

fn polygon_edit_error(
    floor: &super::definition::NavigationFloorDefinition,
) -> Option<String> {
    let polygon = &floor.walkable_outline;
    if polygon.vertices_xz.len() < MIN_VERTEX_COUNT {
        return Some("floor polygon needs at least three vertices".into());
    }
    for window in polygon.vertices_xz.windows(2) {
        let [ax, az] = window[0];
        let [bx, bz] = window[1];
        let dx = bx - ax;
        let dz = bz - az;
        if dx * dx + dz * dz < MIN_EDGE_LENGTH_SQ {
            return Some("floor edge is too short".into());
        }
    }
    if polygon.signed_area() <= f32::EPSILON {
        return Some("floor polygon is degenerate".into());
    }
    None
}

fn point_in_polygon_local(polygon: &NavigationPolygon2d, point: [f32; 2]) -> bool {
    let verts = &polygon.vertices_xz;
    if verts.len() < 3 {
        return false;
    }
    let point = Vec2::new(point[0], point[1]);
    let mut inside = false;
    let mut j = verts.len() - 1;
    for (index, vertex) in verts.iter().enumerate() {
        let vi = Vec2::new(vertex[0], vertex[1]);
        let vj = Vec2::new(verts[j][0], verts[j][1]);
        if ((vi.y > point.y) != (vj.y > point.y))
            && (point.x < (vj.x - vi.x) * (point.y - vi.y) / (vj.y - vi.y + f32::EPSILON) + vi.x)
        {
            inside = !inside;
        }
        j = index;
    }
    inside
}

fn next_feature_key<'a>(existing: impl Iterator<Item = &'a str>, prefix: &str) -> String {
    let existing: Vec<&str> = existing.collect();
    let mut index = 1_u32;
    loop {
        let candidate = format!("{prefix}_{index}");
        if existing.iter().all(|key| *key != candidate) {
            return candidate;
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::starter::two_story_hut_navigation_blueprint;

    #[test]
    fn move_vertex_updates_outline() {
        let mut blueprint = two_story_hut_navigation_blueprint();
        let floor_id = blueprint.floors[0].floor_id;
        let outcome = move_floor_vertex(&mut blueprint, floor_id, 0, [0.5, 0.5]);
        assert!(outcome.applied);
    }

    #[test]
    fn cannot_delete_below_three_vertices() {
        let mut blueprint = two_story_hut_navigation_blueprint();
        let floor_id = blueprint.floors[0].floor_id;
        assert!(delete_floor_vertex(&mut blueprint, floor_id, 0).applied);
        assert!(!delete_floor_vertex(&mut blueprint, floor_id, 0).applied);
    }
}
