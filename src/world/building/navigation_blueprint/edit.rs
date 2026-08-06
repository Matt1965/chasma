//! Dev blueprint edit operations (NV1.4). Pure data mutations with lightweight guards.

use bevy::prelude::Vec2;

use super::definition::{
    BuildingNavigationBlueprint, MIN_CONNECTION_RADIUS, NavigationEntranceDefinition,
    NavigationPolygon2d, NavigationRegionConnectionDefinition, NavigationRegionConnectionKind,
    NavigationRegionDefinition, NavigationVerticalTransitionDefinition,
    NavigationVerticalTransitionKind,
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
    region_key: Option<&str>,
    vertex_index: usize,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint
        .floors
        .iter_mut()
        .find(|floor| floor.floor_id == floor_id)
    else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    let floor_key = floor.key.clone();
    let Some(region) = resolve_region_mut(floor, region_key) else {
        return BlueprintEditOutcome::rejected(
            "select a region before editing vertices on a multi-region floor",
        );
    };
    let region_key_owned = region.key.clone();
    let edit_result = {
        let Some(vertex) = region.walkable_outline.vertices_xz.get_mut(vertex_index) else {
            return BlueprintEditOutcome::rejected("vertex not found");
        };
        if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
            return BlueprintEditOutcome::rejected("vertex position must be finite");
        }
        *vertex = local_xz;
        region_polygon_edit_error(floor, &region_key_owned)
    };
    if let Some(message) = edit_result {
        return BlueprintEditOutcome::rejected(message);
    }
    super::entrance_geometry::reanchor_entrances_after_region_edit(
        blueprint,
        &floor_key,
        &region_key_owned,
    );
    BlueprintEditOutcome::ok()
}

pub fn insert_vertex_on_edge(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    region_key: Option<&str>,
    edge_index: usize,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint
        .floors
        .iter_mut()
        .find(|floor| floor.floor_id == floor_id)
    else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    let floor_key = floor.key.clone();
    let Some(region) = resolve_region_mut(floor, region_key) else {
        return BlueprintEditOutcome::rejected(
            "select a region before editing vertices on a multi-region floor",
        );
    };
    let region_key_owned = region.key.clone();
    let vertex_count = region.walkable_outline.vertices_xz.len();
    if vertex_count < MIN_VERTEX_COUNT || edge_index >= vertex_count {
        return BlueprintEditOutcome::rejected("invalid edge");
    }
    if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
        return BlueprintEditOutcome::rejected("vertex position must be finite");
    }
    let edit_result = {
        region
            .walkable_outline
            .vertices_xz
            .insert(edge_index + 1, local_xz);
        region_polygon_edit_error(floor, &region_key_owned)
    };
    if let Some(message) = edit_result {
        if let Some(region) = floor.region_by_key_mut(&region_key_owned) {
            region.walkable_outline.vertices_xz.remove(edge_index + 1);
        }
        return BlueprintEditOutcome::rejected(message);
    }
    super::entrance_geometry::reanchor_entrances_after_region_edit(
        blueprint,
        &floor_key,
        &region_key_owned,
    );
    BlueprintEditOutcome::ok()
}

pub fn delete_floor_vertex(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    region_key: Option<&str>,
    vertex_index: usize,
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint
        .floors
        .iter_mut()
        .find(|floor| floor.floor_id == floor_id)
    else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    let floor_key = floor.key.clone();
    let Some(region) = resolve_region_mut(floor, region_key) else {
        return BlueprintEditOutcome::rejected(
            "select a region before editing vertices on a multi-region floor",
        );
    };
    let region_key_owned = region.key.clone();
    if region.walkable_outline.vertices_xz.len() <= MIN_VERTEX_COUNT {
        return BlueprintEditOutcome::rejected("floor polygon must keep at least three vertices");
    }
    if vertex_index >= region.walkable_outline.vertices_xz.len() {
        return BlueprintEditOutcome::rejected("vertex not found");
    }
    let edit_result = {
        region.walkable_outline.vertices_xz.remove(vertex_index);
        region_polygon_edit_error(floor, &region_key_owned)
    };
    if let Some(message) = edit_result {
        return BlueprintEditOutcome::rejected(message);
    }
    super::entrance_geometry::reanchor_entrances_after_region_edit(
        blueprint,
        &floor_key,
        &region_key_owned,
    );
    BlueprintEditOutcome::ok()
}

pub fn move_entrance(
    blueprint: &mut BuildingNavigationBlueprint,
    entrance_key: &str,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
        return BlueprintEditOutcome::rejected("entrance position must be finite");
    }
    let entrance_meta = blueprint
        .entrances
        .iter()
        .find(|entrance| entrance.key == entrance_key)
        .map(|entrance| (entrance.floor_key.clone(), entrance.region_key.clone()));
    let Some((floor_key, region_key_hint)) = entrance_meta else {
        return BlueprintEditOutcome::rejected("entrance not found");
    };
    let Some(floor) = blueprint.floor_by_key(&floor_key) else {
        return BlueprintEditOutcome::rejected("entrance floor missing");
    };
    let region_key =
        match blueprint.resolve_region_key(&floor_key, region_key_hint.as_deref(), entrance_key) {
            Ok(key) => key,
            Err(err) => return BlueprintEditOutcome::rejected(err.to_string()),
        };
    let Some(region) = floor.region_by_key(region_key) else {
        return BlueprintEditOutcome::rejected("entrance region missing");
    };
    let vertices = region.walkable_outline.vertices_xz.clone();
    let elevation = floor.elevation_meters;
    let pointer = Vec2::new(local_xz[0], local_xz[1]);
    let Some(projection) = super::entrance_geometry::nearest_boundary_projection(
        &vertices,
        pointer,
        super::entrance_geometry::ENTRANCE_EDGE_SNAP_MAX_DISTANCE,
        super::entrance_geometry::ENTRANCE_CORNER_MARGIN,
    ) else {
        return BlueprintEditOutcome::rejected("Move the cursor near a region boundary.");
    };
    let Some(entrance) = blueprint
        .entrances
        .iter_mut()
        .find(|entrance| entrance.key == entrance_key)
    else {
        return BlueprintEditOutcome::rejected("entrance not found");
    };
    super::entrance_geometry::apply_threshold_geometry(
        entrance,
        elevation,
        &projection,
        super::entrance_geometry::DEFAULT_INTERIOR_LANDING_OFFSET,
    );
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
    region_key: Option<&str>,
    local_xz: [f32; 2],
    radius_meters: f32,
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint.floor_by_key(floor_key) else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    if !(radius_meters > 0.0) || !radius_meters.is_finite() {
        return BlueprintEditOutcome::rejected("entrance radius must be positive and finite");
    }
    let resolved_region_key = match region_key {
        Some(key) => key.to_string(),
        None => match floor.single_region_key() {
            Some(key) => key.to_string(),
            None => {
                return BlueprintEditOutcome::rejected(
                    "multi-region floor requires an explicit entrance target region",
                );
            }
        },
    };
    let Some(region) = floor.region_by_key(&resolved_region_key) else {
        return BlueprintEditOutcome::rejected("entrance target region not found");
    };
    let pointer = Vec2::new(local_xz[0], local_xz[1]);
    let Some(projection) = super::entrance_geometry::nearest_boundary_projection(
        &region.walkable_outline.vertices_xz,
        pointer,
        super::entrance_geometry::ENTRANCE_EDGE_SNAP_MAX_DISTANCE,
        super::entrance_geometry::ENTRANCE_CORNER_MARGIN,
    ) else {
        return BlueprintEditOutcome::rejected("Place entrances on a region boundary.");
    };
    let elevation_meters = floor.elevation_meters;
    let key = next_feature_key(all_feature_keys(blueprint).into_iter(), "entrance");
    let mut entrance = NavigationEntranceDefinition {
        key,
        floor_key: floor_key.to_string(),
        region_key: Some(resolved_region_key),
        local_position_xz: [0.0, 0.0],
        radius_meters,
        interior_spawn_local: [0.0, elevation_meters, 0.0],
        bidirectional: true,
        door_key: None,
    };
    super::entrance_geometry::apply_threshold_geometry(
        &mut entrance,
        elevation_meters,
        &projection,
        super::entrance_geometry::DEFAULT_INTERIOR_LANDING_OFFSET,
    );
    blueprint.entrances.push(entrance);
    BlueprintEditOutcome::ok()
}

pub fn delete_entrance(
    blueprint: &mut BuildingNavigationBlueprint,
    entrance_key: &str,
) -> BlueprintEditOutcome {
    let before = blueprint.entrances.len();
    blueprint
        .entrances
        .retain(|entrance| entrance.key != entrance_key);
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
    let key = next_feature_key(all_feature_keys(blueprint).into_iter(), "stairs");
    let from_region_key = blueprint
        .floor_by_key(from_floor_key)
        .and_then(|floor| floor.single_region_key())
        .map(str::to_string);
    let to_region_key = blueprint
        .floor_by_key(to_floor_key)
        .and_then(|floor| floor.single_region_key())
        .map(str::to_string);
    blueprint
        .vertical_transitions
        .push(NavigationVerticalTransitionDefinition {
            key,
            kind: NavigationVerticalTransitionKind::Stair,
            from_floor_key: from_floor_key.to_string(),
            to_floor_key: to_floor_key.to_string(),
            from_region_key,
            to_region_key,
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

/// References blocking deletion of a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionReference {
    pub feature_kind: &'static str,
    pub feature_key: String,
}

pub fn region_references(
    blueprint: &BuildingNavigationBlueprint,
    floor_key: &str,
    region_key: &str,
) -> Vec<RegionReference> {
    let mut refs = Vec::new();
    for entrance in &blueprint.entrances {
        if entrance.floor_key == floor_key && entrance.region_key.as_deref() == Some(region_key) {
            refs.push(RegionReference {
                feature_kind: "entrance",
                feature_key: entrance.key.clone(),
            });
        }
    }
    for transition in &blueprint.vertical_transitions {
        if transition.from_floor_key == floor_key
            && transition.from_region_key.as_deref() == Some(region_key)
        {
            refs.push(RegionReference {
                feature_kind: "vertical transition",
                feature_key: transition.key.clone(),
            });
        }
        if transition.to_floor_key == floor_key
            && transition.to_region_key.as_deref() == Some(region_key)
        {
            refs.push(RegionReference {
                feature_kind: "vertical transition",
                feature_key: transition.key.clone(),
            });
        }
    }
    for connection in &blueprint.region_connections {
        if connection.floor_key != floor_key {
            continue;
        }
        if connection.from_region_key == region_key || connection.to_region_key == region_key {
            refs.push(RegionReference {
                feature_kind: "connection",
                feature_key: connection.key.clone(),
            });
        }
    }
    refs
}

pub fn format_region_deletion_error(region_key: &str, references: &[RegionReference]) -> String {
    if references.is_empty() {
        return format!("Cannot delete region \"{region_key}\".");
    }
    let keys = references
        .iter()
        .map(|reference| format!("{} \"{}\"", reference.feature_kind, reference.feature_key))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Cannot delete region \"{region_key}\": referenced by {keys}.")
}

pub fn add_region_on_floor(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    local_offset_xz: Option<[f32; 2]>,
) -> Result<String, String> {
    let Some(floor) = blueprint
        .floors
        .iter_mut()
        .find(|floor| floor.floor_id == floor_id)
    else {
        return Err("floor not found".into());
    };
    let key = next_region_key(floor);
    let [offset_x, offset_z] = local_offset_xz.unwrap_or_else(|| {
        let index = floor.regions.len() as f32;
        [index * 5.0, 0.0]
    });
    let mut outline = NavigationPolygon2d::rectangle(4.0, 4.0);
    for vertex in &mut outline.vertices_xz {
        vertex[0] += offset_x;
        vertex[1] += offset_z;
    }
    let display_label = format!("Region {}", floor.regions.len() + 1);
    floor.regions.push(NavigationRegionDefinition {
        key: key.clone(),
        display_label,
        room_tag: floor.room_tag.clone(),
        walkable_outline: outline,
    });
    Ok(key)
}

pub fn delete_region(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    region_key: &str,
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint
        .floors
        .iter()
        .find(|floor| floor.floor_id == floor_id)
    else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    if floor.regions.len() <= 1 {
        return BlueprintEditOutcome::rejected("floor must keep at least one region");
    }
    let references = region_references(blueprint, &floor.key, region_key);
    if !references.is_empty() {
        return BlueprintEditOutcome::rejected(format_region_deletion_error(
            region_key,
            &references,
        ));
    }
    let Some(floor) = blueprint
        .floors
        .iter_mut()
        .find(|floor| floor.floor_id == floor_id)
    else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    let before = floor.regions.len();
    floor.regions.retain(|region| region.key != region_key);
    if floor.regions.len() == before {
        return BlueprintEditOutcome::rejected("region not found");
    }
    BlueprintEditOutcome::ok()
}

pub fn set_region_display_label(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    region_key: &str,
    display_label: &str,
) -> BlueprintEditOutcome {
    if display_label.trim().is_empty() {
        return BlueprintEditOutcome::rejected("region display label must not be empty");
    }
    let Some(floor) = blueprint
        .floors
        .iter_mut()
        .find(|floor| floor.floor_id == floor_id)
    else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    let Some(region) = floor.region_by_key_mut(region_key) else {
        return BlueprintEditOutcome::rejected("region not found");
    };
    region.display_label = display_label.trim().to_string();
    BlueprintEditOutcome::ok()
}

pub fn set_region_room_tag(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_id: i32,
    region_key: &str,
    room_tag: Option<String>,
) -> BlueprintEditOutcome {
    let Some(floor) = blueprint
        .floors
        .iter_mut()
        .find(|floor| floor.floor_id == floor_id)
    else {
        return BlueprintEditOutcome::rejected("floor not found");
    };
    let Some(region) = floor.region_by_key_mut(region_key) else {
        return BlueprintEditOutcome::rejected("region not found");
    };
    region.room_tag = room_tag.filter(|tag| !tag.trim().is_empty());
    BlueprintEditOutcome::ok()
}

pub fn set_entrance_region_key(
    blueprint: &mut BuildingNavigationBlueprint,
    entrance_key: &str,
    region_key: Option<&str>,
) -> BlueprintEditOutcome {
    let floor_key = blueprint
        .entrances
        .iter()
        .find(|entrance| entrance.key == entrance_key)
        .map(|entrance| entrance.floor_key.clone());
    let Some(floor_key) = floor_key else {
        return BlueprintEditOutcome::rejected("entrance not found");
    };
    let Some(floor) = blueprint.floor_by_key(&floor_key) else {
        return BlueprintEditOutcome::rejected("entrance floor not found");
    };
    let resolved = match region_key {
        Some(key) => key.to_string(),
        None => match floor.single_region_key() {
            Some(key) => key.to_string(),
            None => {
                return BlueprintEditOutcome::rejected(
                    "multi-region floor requires an explicit entrance target region",
                );
            }
        },
    };
    if floor.region_by_key(&resolved).is_none() {
        return BlueprintEditOutcome::rejected("entrance target region not found");
    }
    let Some(entrance) = blueprint
        .entrances
        .iter_mut()
        .find(|entrance| entrance.key == entrance_key)
    else {
        return BlueprintEditOutcome::rejected("entrance not found");
    };
    entrance.region_key = Some(resolved);
    BlueprintEditOutcome::ok()
}

pub fn add_region_connection(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_key: &str,
    from_region_key: &str,
    to_region_key: &str,
    from_local_xz: [f32; 2],
    to_local_xz: [f32; 2],
    radius_meters: f32,
) -> Result<String, String> {
    if from_region_key == to_region_key {
        return Err("connection source and destination region must differ".into());
    }
    let Some(floor) = blueprint.floor_by_key(floor_key) else {
        return Err("floor not found".into());
    };
    if floor.region_by_key(from_region_key).is_none()
        || floor.region_by_key(to_region_key).is_none()
    {
        return Err("connection region not found on floor".into());
    }
    if !(radius_meters >= MIN_CONNECTION_RADIUS) || !radius_meters.is_finite() {
        return Err(format!(
            "connection radius must be at least {MIN_CONNECTION_RADIUS} meters"
        ));
    }
    let key = next_feature_key(all_feature_keys(blueprint).into_iter(), "connection");
    blueprint
        .region_connections
        .push(NavigationRegionConnectionDefinition {
            key: key.clone(),
            kind: NavigationRegionConnectionKind::Doorway,
            floor_key: floor_key.to_string(),
            from_region_key: from_region_key.to_string(),
            to_region_key: to_region_key.to_string(),
            from_local_position_xz: from_local_xz,
            to_local_position_xz: to_local_xz,
            radius_meters,
            bidirectional: true,
            enabled: true,
            door_key: None,
        });
    Ok(key)
}

pub fn delete_region_connection(
    blueprint: &mut BuildingNavigationBlueprint,
    connection_key: &str,
) -> BlueprintEditOutcome {
    let before = blueprint.region_connections.len();
    blueprint
        .region_connections
        .retain(|connection| connection.key != connection_key);
    if blueprint.region_connections.len() == before {
        return BlueprintEditOutcome::rejected("connection not found");
    }
    BlueprintEditOutcome::ok()
}

pub fn move_connection_from(
    blueprint: &mut BuildingNavigationBlueprint,
    connection_key: &str,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    let Some(connection) = blueprint
        .region_connections
        .iter_mut()
        .find(|connection| connection.key == connection_key)
    else {
        return BlueprintEditOutcome::rejected("connection not found");
    };
    if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
        return BlueprintEditOutcome::rejected("connection position must be finite");
    }
    connection.from_local_position_xz = local_xz;
    BlueprintEditOutcome::ok()
}

pub fn move_connection_to(
    blueprint: &mut BuildingNavigationBlueprint,
    connection_key: &str,
    local_xz: [f32; 2],
) -> BlueprintEditOutcome {
    let Some(connection) = blueprint
        .region_connections
        .iter_mut()
        .find(|connection| connection.key == connection_key)
    else {
        return BlueprintEditOutcome::rejected("connection not found");
    };
    if !local_xz[0].is_finite() || !local_xz[1].is_finite() {
        return BlueprintEditOutcome::rejected("connection position must be finite");
    }
    connection.to_local_position_xz = local_xz;
    BlueprintEditOutcome::ok()
}

pub fn set_connection_radius(
    blueprint: &mut BuildingNavigationBlueprint,
    connection_key: &str,
    radius_meters: f32,
) -> BlueprintEditOutcome {
    if !(radius_meters >= MIN_CONNECTION_RADIUS) || !radius_meters.is_finite() {
        return BlueprintEditOutcome::rejected(format!(
            "connection radius must be at least {MIN_CONNECTION_RADIUS} meters"
        ));
    }
    let Some(connection) = blueprint
        .region_connections
        .iter_mut()
        .find(|connection| connection.key == connection_key)
    else {
        return BlueprintEditOutcome::rejected("connection not found");
    };
    connection.radius_meters = radius_meters;
    BlueprintEditOutcome::ok()
}

pub fn set_connection_kind(
    blueprint: &mut BuildingNavigationBlueprint,
    connection_key: &str,
    kind: NavigationRegionConnectionKind,
) -> BlueprintEditOutcome {
    let Some(connection) = blueprint
        .region_connections
        .iter_mut()
        .find(|connection| connection.key == connection_key)
    else {
        return BlueprintEditOutcome::rejected("connection not found");
    };
    if kind == NavigationRegionConnectionKind::OpenArch {
        connection.door_key = None;
    }
    connection.kind = kind;
    BlueprintEditOutcome::ok()
}

pub fn set_connection_door_key(
    blueprint: &mut BuildingNavigationBlueprint,
    connection_key: &str,
    door_key: Option<String>,
) -> BlueprintEditOutcome {
    let Some(connection) = blueprint
        .region_connections
        .iter_mut()
        .find(|connection| connection.key == connection_key)
    else {
        return BlueprintEditOutcome::rejected("connection not found");
    };
    if connection.kind == NavigationRegionConnectionKind::OpenArch && door_key.is_some() {
        return BlueprintEditOutcome::rejected("OpenArch connections cannot have a door key");
    }
    connection.door_key = door_key.filter(|key| !key.trim().is_empty());
    BlueprintEditOutcome::ok()
}

pub fn set_connection_bidirectional(
    blueprint: &mut BuildingNavigationBlueprint,
    connection_key: &str,
    bidirectional: bool,
) -> BlueprintEditOutcome {
    let Some(connection) = blueprint
        .region_connections
        .iter_mut()
        .find(|connection| connection.key == connection_key)
    else {
        return BlueprintEditOutcome::rejected("connection not found");
    };
    connection.bidirectional = bidirectional;
    BlueprintEditOutcome::ok()
}

pub fn set_connection_enabled(
    blueprint: &mut BuildingNavigationBlueprint,
    connection_key: &str,
    enabled: bool,
) -> BlueprintEditOutcome {
    let Some(connection) = blueprint
        .region_connections
        .iter_mut()
        .find(|connection| connection.key == connection_key)
    else {
        return BlueprintEditOutcome::rejected("connection not found");
    };
    connection.enabled = enabled;
    BlueprintEditOutcome::ok()
}

pub fn region_interior_point(
    blueprint: &BuildingNavigationBlueprint,
    floor_key: &str,
    region_key: &str,
) -> Option<[f32; 2]> {
    let floor = blueprint.floor_by_key(floor_key)?;
    let region = floor.region_by_key(region_key)?;
    let verts = &region.walkable_outline.vertices_xz;
    if verts.is_empty() {
        return None;
    }
    let sum = verts
        .iter()
        .fold(Vec2::ZERO, |acc, &[x, z]| acc + Vec2::new(x, z));
    let centroid = sum / verts.len() as f32;
    Some([centroid.x, centroid.y])
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

fn resolve_region_mut<'a>(
    floor: &'a mut super::definition::NavigationFloorDefinition,
    region_key: Option<&str>,
) -> Option<&'a mut NavigationRegionDefinition> {
    match region_key {
        Some(key) => floor.region_by_key_mut(key),
        None => floor.sole_region_mut(),
    }
}

fn all_feature_keys(blueprint: &BuildingNavigationBlueprint) -> Vec<&str> {
    blueprint
        .entrances
        .iter()
        .map(|entrance| entrance.key.as_str())
        .chain(
            blueprint
                .vertical_transitions
                .iter()
                .map(|transition| transition.key.as_str()),
        )
        .chain(
            blueprint
                .region_connections
                .iter()
                .map(|connection| connection.key.as_str()),
        )
        .collect()
}

fn next_region_key(floor: &super::definition::NavigationFloorDefinition) -> String {
    let existing: Vec<&str> = floor
        .regions
        .iter()
        .map(|region| region.key.as_str())
        .collect();
    let mut index = 1_u32;
    loop {
        let candidate = if index == 1 {
            "region".to_string()
        } else {
            format!("region_{index}")
        };
        if existing.iter().all(|key| *key != candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn region_polygon_edit_error(
    floor: &super::definition::NavigationFloorDefinition,
    region_key: &str,
) -> Option<String> {
    let polygon = floor.region_by_key(region_key)?.walkable_outline.clone();
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

fn polygon_edit_error(floor: &super::definition::NavigationFloorDefinition) -> Option<String> {
    let region_key = floor.single_region_key()?;
    region_polygon_edit_error(floor, region_key)
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
    use crate::world::building::navigation_blueprint::fixtures::two_room_hut_navigation_blueprint;
    use crate::world::building::navigation_blueprint::starter::two_story_hut_navigation_blueprint;

    #[test]
    fn move_vertex_updates_outline() {
        let mut blueprint = two_story_hut_navigation_blueprint();
        let floor_id = blueprint.floors[0].floor_id;
        let outcome = move_floor_vertex(&mut blueprint, floor_id, None, 0, [0.5, 0.5]);
        assert!(outcome.applied);
    }

    #[test]
    fn cannot_delete_below_three_vertices() {
        let mut blueprint = two_story_hut_navigation_blueprint();
        let floor_id = blueprint.floors[0].floor_id;
        assert!(delete_floor_vertex(&mut blueprint, floor_id, None, 0).applied);
        assert!(!delete_floor_vertex(&mut blueprint, floor_id, None, 0).applied);
    }

    #[test]
    fn add_region_creates_unique_key_and_three_vertices() {
        let mut blueprint = two_room_hut_navigation_blueprint();
        let floor_id = blueprint.floors[0].floor_id;
        let before = blueprint.floors[0].regions.len();
        let connections_before = blueprint.region_connections.len();
        let key = add_region_on_floor(&mut blueprint, floor_id, None).expect("region");
        assert_eq!(blueprint.floors[0].regions.len(), before + 1);
        let region = blueprint.floors[0].region_by_key(&key).expect("new region");
        assert!(region.walkable_outline.vertices_xz.len() >= 3);
        assert_eq!(blueprint.region_connections.len(), connections_before);
    }

    #[test]
    fn move_vertex_only_changes_selected_region() {
        let mut blueprint = two_room_hut_navigation_blueprint();
        let floor_id = blueprint.floors[0].floor_id;
        let room_b_before = blueprint.floors[0]
            .region_by_key("room_b")
            .expect("room_b")
            .walkable_outline
            .vertices_xz
            .clone();
        assert!(move_floor_vertex(&mut blueprint, floor_id, Some("room_a"), 0, [0.5, 0.5]).applied);
        let room_b_after = blueprint.floors[0]
            .region_by_key("room_b")
            .expect("room_b")
            .walkable_outline
            .vertices_xz
            .clone();
        assert_eq!(room_b_before, room_b_after);
    }

    #[test]
    fn referenced_region_deletion_lists_feature_keys() {
        let blueprint = two_room_hut_navigation_blueprint();
        let refs = region_references(&blueprint, "ground", "room_a");
        assert!(
            refs.iter()
                .any(|reference| reference.feature_key == "exterior_entrance")
        );
        let message = format_region_deletion_error("room_a", &refs);
        assert!(message.contains("exterior_entrance"));
        assert!(!delete_region(&mut blueprint.clone(), 0, "room_a").applied);
    }

    #[test]
    fn region_connection_creation_defaults() {
        let mut blueprint = two_room_hut_navigation_blueprint();
        blueprint.region_connections.clear();
        let key = add_region_connection(
            &mut blueprint,
            "ground",
            "room_a",
            "room_b",
            [5.7, 2.0],
            [6.7, 2.0],
            0.8,
        )
        .expect("connection");
        let connection = blueprint
            .region_connections
            .iter()
            .find(|connection| connection.key == key)
            .expect("connection");
        assert_eq!(connection.kind, NavigationRegionConnectionKind::Doorway);
        assert!(connection.bidirectional);
        assert!(connection.enabled);
        assert!(connection.door_key.is_none());
    }

    #[test]
    fn open_arch_clears_door_key() {
        let mut blueprint = two_room_hut_navigation_blueprint();
        let key = blueprint.region_connections[0].key.clone();
        assert!(set_connection_door_key(&mut blueprint, &key, Some("door_a".into())).applied);
        assert!(
            set_connection_kind(
                &mut blueprint,
                &key,
                NavigationRegionConnectionKind::OpenArch
            )
            .applied
        );
        assert!(blueprint.region_connections[0].door_key.is_none());
        assert!(!set_connection_door_key(&mut blueprint, &key, Some("door_a".into())).applied);
    }

    #[test]
    fn entrance_retarget_does_not_move_spawn() {
        let mut blueprint = two_room_hut_navigation_blueprint();
        let spawn_before = blueprint.entrances[0].interior_spawn_local;
        assert!(
            set_entrance_region_key(&mut blueprint, "exterior_entrance", Some("room_b")).applied
        );
        assert_eq!(blueprint.entrances[0].interior_spawn_local, spawn_before);
        assert_eq!(blueprint.entrances[0].region_key.as_deref(), Some("room_b"));
    }
}
