//! Extended blueprint validation for dev inspection (NV1.2.5 + NV2).

use super::definition::{
    BuildingNavigationBlueprint, MIN_CONNECTION_RADIUS, MIN_REGION_AREA,
    NavigationEntranceDefinition, NavigationFloorDefinition, NavigationRegionConnectionDefinition,
    NavigationRegionConnectionKind, NavigationVerticalTransitionDefinition, point_inside_polygon,
};
use super::id::BuildingNavigationBlueprintId;
use bevy::prelude::Vec2;

const DUPLICATE_VERTEX_EPSILON: f32 = 0.05;
const MIN_EDGE_LENGTH: f32 = 0.1;
const REGION_ENDPOINT_INSET_MIN: f32 = 0.15;
const TYPICAL_AGENT_RADIUS: f32 = 0.5;
const REGION_TOUCH_EPSILON: f32 = 0.05;
const CONNECTION_ENDPOINT_MAX_DISTANCE: f32 = 3.0;
const FLOOR_ELEVATION_TOLERANCE_METERS: f32 = 1.5;

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
    pub connection_key: Option<String>,
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
        if floor.regions.is_empty() {
            push_error(
                &mut diagnostics,
                "floor_has_no_regions",
                format!("floor {} has no regions", floor.floor_id),
                BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    ..Default::default()
                },
            );
        }
        validate_floor_regions(floor, blueprint, &mut diagnostics);
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
    for connection in &blueprint.region_connections {
        if !feature_keys.insert(connection.key.clone()) {
            push_error(
                &mut diagnostics,
                "duplicate_connection_key",
                format!("duplicate connection key `{}`", connection.key),
                BlueprintDiagnosticFocus {
                    connection_key: Some(connection.key.clone()),
                    ..Default::default()
                },
            );
        }
        validate_connection_inspection(connection, blueprint, &mut diagnostics);
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
            blueprint.id, blueprint.schema_version, blueprint.metadata.generation_revision
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
            connection_key: None,
        }
    }
}

fn validate_floor_regions(
    floor: &NavigationFloorDefinition,
    blueprint: &BuildingNavigationBlueprint,
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
) {
    let mut region_keys = std::collections::BTreeSet::new();
    for region in &floor.regions {
        if !region_keys.insert(region.key.clone()) {
            push_error(
                diagnostics,
                "duplicate_region_key",
                format!(
                    "duplicate region key `{}` on floor {}",
                    region.key, floor.floor_id
                ),
                BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    ..Default::default()
                },
            );
        }
        validate_region_polygon(floor, region, diagnostics);
    }

    for i in 0..floor.regions.len() {
        for j in (i + 1)..floor.regions.len() {
            let a = &floor.regions[i];
            let b = &floor.regions[j];
            if regions_overlap(
                &a.walkable_outline.vertices_xz,
                &b.walkable_outline.vertices_xz,
            ) {
                push_error(
                    diagnostics,
                    "region_overlap",
                    format!(
                        "regions `{}` and `{}` on floor {} overlap",
                        a.key, b.key, floor.floor_id
                    ),
                    BlueprintDiagnosticFocus {
                        floor_id: Some(floor.floor_id),
                        ..Default::default()
                    },
                );
            } else if region_contains(
                &a.walkable_outline.vertices_xz,
                &b.walkable_outline.vertices_xz,
            ) {
                push_error(
                    diagnostics,
                    "region_containment",
                    format!(
                        "region `{}` on floor {} is contained within `{}`",
                        b.key, floor.floor_id, a.key
                    ),
                    BlueprintDiagnosticFocus {
                        floor_id: Some(floor.floor_id),
                        ..Default::default()
                    },
                );
            } else if region_contains(
                &b.walkable_outline.vertices_xz,
                &a.walkable_outline.vertices_xz,
            ) {
                push_error(
                    diagnostics,
                    "region_containment",
                    format!(
                        "region `{}` on floor {} is contained within `{}`",
                        a.key, floor.floor_id, b.key
                    ),
                    BlueprintDiagnosticFocus {
                        floor_id: Some(floor.floor_id),
                        ..Default::default()
                    },
                );
            } else if regions_touching(
                &a.walkable_outline.vertices_xz,
                &b.walkable_outline.vertices_xz,
            ) {
                push_info(
                    diagnostics,
                    "region_touching",
                    format!(
                        "regions `{}` and `{}` on floor {} touch or are very close",
                        a.key, b.key, floor.floor_id
                    ),
                    Some(BlueprintDiagnosticFocus {
                        floor_id: Some(floor.floor_id),
                        ..Default::default()
                    }),
                );
            }
        }
    }

    validate_disconnected_regions(floor, blueprint, diagnostics);
}

fn validate_region_polygon(
    floor: &NavigationFloorDefinition,
    region: &super::definition::NavigationRegionDefinition,
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
) {
    let focus_floor = BlueprintDiagnosticFocus {
        floor_id: Some(floor.floor_id),
        ..Default::default()
    };
    let verts = &region.walkable_outline.vertices_xz;
    if verts.len() < 3 {
        push_error(
            diagnostics,
            "region_too_few_vertices",
            format!(
                "region {} on floor {} has fewer than three vertices",
                region.key, floor.floor_id
            ),
            focus_floor.clone(),
        );
        return;
    }

    let mut unique: Vec<(Vec2, usize)> = Vec::new();
    for (index, [x, z]) in verts.iter().enumerate() {
        if !x.is_finite() || !z.is_finite() {
            push_error(
                diagnostics,
                "region_non_finite_vertex",
                format!(
                    "region {} on floor {} vertex {index} is non-finite",
                    region.key, floor.floor_id
                ),
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
                "region_duplicate_vertex",
                format!(
                    "region {} on floor {} vertex {index} duplicates an earlier vertex",
                    region.key, floor.floor_id
                ),
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
            format!(
                "region {} on floor {} has fewer than three unique vertices",
                region.key, floor.floor_id
            ),
            focus_floor.clone(),
        );
        return;
    }

    let area = region.walkable_outline.signed_area();
    if area.abs() <= MIN_REGION_AREA {
        push_error(
            diagnostics,
            "region_zero_area",
            format!(
                "region {} on floor {} polygon area is below minimum",
                region.key, floor.floor_id
            ),
            focus_floor.clone(),
        );
    } else if area < 0.0 {
        push_warning(
            diagnostics,
            "region_clockwise_winding",
            format!(
                "region {} on floor {} polygon appears clockwise",
                region.key, floor.floor_id
            ),
            Some(focus_floor.clone()),
        );
    }

    for i in 0..verts.len() {
        let a = Vec2::new(verts[i][0], verts[i][1]);
        let b = Vec2::new(
            verts[(i + 1) % verts.len()][0],
            verts[(i + 1) % verts.len()][1],
        );
        if a.distance(b) < MIN_EDGE_LENGTH {
            push_warning(
                diagnostics,
                "region_short_edge",
                format!(
                    "region {} on floor {} edge {i} is very short",
                    region.key, floor.floor_id
                ),
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
            "region_self_intersection",
            format!(
                "region {} on floor {} polygon self-intersects",
                region.key, floor.floor_id
            ),
            focus_floor,
        );
    }
}

fn validate_disconnected_regions(
    floor: &NavigationFloorDefinition,
    blueprint: &BuildingNavigationBlueprint,
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
) {
    if floor.regions.len() <= 1 {
        return;
    }
    let mut adjacency: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for region in &floor.regions {
        adjacency.insert(region.key.clone(), Vec::new());
    }
    for connection in &blueprint.region_connections {
        if connection.floor_key != floor.key {
            continue;
        }
        if connection.bidirectional {
            adjacency
                .entry(connection.from_region_key.clone())
                .or_default()
                .push(connection.to_region_key.clone());
            adjacency
                .entry(connection.to_region_key.clone())
                .or_default()
                .push(connection.from_region_key.clone());
        } else {
            adjacency
                .entry(connection.from_region_key.clone())
                .or_default()
                .push(connection.to_region_key.clone());
        }
    }
    let seed = blueprint
        .entrances
        .iter()
        .find_map(|entrance| {
            if entrance.floor_key != floor.key {
                return None;
            }
            blueprint
                .resolve_region_key(
                    &entrance.floor_key,
                    entrance.region_key.as_deref(),
                    &entrance.key,
                )
                .ok()
                .map(str::to_string)
        })
        .or_else(|| floor.regions.first().map(|region| region.key.clone()));
    let Some(seed) = seed else {
        return;
    };
    let mut reachable = std::collections::BTreeSet::new();
    let mut queue = std::collections::VecDeque::from([seed]);
    while let Some(key) = queue.pop_front() {
        if !reachable.insert(key.clone()) {
            continue;
        }
        if let Some(neighbors) = adjacency.get(&key) {
            for neighbor in neighbors {
                if !reachable.contains(neighbor) {
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }
    for region in &floor.regions {
        if !reachable.contains(&region.key) {
            push_warning(
                diagnostics,
                "region_disconnected",
                format!(
                    "region `{}` on floor {} is not reachable from entrance-connected regions",
                    region.key, floor.floor_id
                ),
                Some(BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    ..Default::default()
                }),
            );
        }
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
    let region_key = match blueprint.resolve_region_key(
        &entrance.floor_key,
        entrance.region_key.as_deref(),
        &entrance.key,
    ) {
        Ok(key) => key,
        Err(err) => {
            let code = if matches!(
                err,
                super::error::BuildingNavigationBlueprintError::RegionReferenceAmbiguous { .. }
                    | super::error::BuildingNavigationBlueprintError::EntranceRegionAmbiguous { .. }
            ) {
                "entrance_region_ambiguous"
            } else {
                "entrance_region_missing"
            };
            push_error(
                diagnostics,
                code,
                err.to_string(),
                BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    entrance_key: Some(entrance.key.clone()),
                    ..Default::default()
                },
            );
            return;
        }
    };
    let region = floor.region_by_key(region_key).expect("resolved");
    let spawn_xz = Vec2::new(
        entrance.interior_spawn_local[0],
        entrance.interior_spawn_local[2],
    );
    if !point_inside_polygon(&region.walkable_outline.vertices_xz, spawn_xz) {
        push_error(
            diagnostics,
            "entrance_spawn_outside_region",
            format!(
                "entrance `{}` spawn lies outside region `{}`",
                entrance.key, region.key
            ),
            BlueprintDiagnosticFocus {
                floor_id: Some(floor.floor_id),
                entrance_key: Some(entrance.key.clone()),
                ..Default::default()
            },
        );
    } else if point_near_boundary(&region.walkable_outline.vertices_xz, spawn_xz) {
        push_warning(
            diagnostics,
            "entrance_spawn_near_boundary",
            format!(
                "entrance `{}` spawn is near region `{}` boundary",
                entrance.key, region.key
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
    if transition.from_floor_key == transition.to_floor_key {
        push_error(
            diagnostics,
            "transition_same_floor",
            format!(
                "transition `{}` connects regions on the same floor",
                transition.key
            ),
            focus.clone(),
        );
        return;
    }
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
    let (Some(from_floor), Some(to_floor)) = (from, to) else {
        return;
    };
    let from_region_key = match blueprint.resolve_region_key(
        &transition.from_floor_key,
        transition.from_region_key.as_deref(),
        &transition.key,
    ) {
        Ok(key) => key,
        Err(err) => {
            push_error(
                diagnostics,
                "transition_region_ambiguous",
                err.to_string(),
                BlueprintDiagnosticFocus {
                    floor_id: Some(from_floor.floor_id),
                    transition_key: Some(transition.key.clone()),
                    ..Default::default()
                },
            );
            return;
        }
    };
    let to_region_key = match blueprint.resolve_region_key(
        &transition.to_floor_key,
        transition.to_region_key.as_deref(),
        &transition.key,
    ) {
        Ok(key) => key,
        Err(err) => {
            push_error(
                diagnostics,
                "transition_region_ambiguous",
                err.to_string(),
                BlueprintDiagnosticFocus {
                    floor_id: Some(to_floor.floor_id),
                    transition_key: Some(transition.key.clone()),
                    ..Default::default()
                },
            );
            return;
        }
    };
    let from_region = from_floor.region_by_key(from_region_key).expect("resolved");
    let to_region = to_floor.region_by_key(to_region_key).expect("resolved");
    let from_pos = Vec2::new(
        transition.from_local_position_xz[0],
        transition.from_local_position_xz[1],
    );
    if !point_inside_polygon(&from_region.walkable_outline.vertices_xz, from_pos) {
        push_error(
            diagnostics,
            "transition_outside_from_region",
            format!(
                "transition `{}` source lies outside region `{}`",
                transition.key, from_region.key
            ),
            BlueprintDiagnosticFocus {
                floor_id: Some(from_floor.floor_id),
                transition_key: Some(transition.key.clone()),
                ..Default::default()
            },
        );
    }
    let to_pos = Vec2::new(
        transition.to_local_position[0],
        transition.to_local_position[2],
    );
    if !point_inside_polygon(&to_region.walkable_outline.vertices_xz, to_pos) {
        push_error(
            diagnostics,
            "transition_outside_to_region",
            format!(
                "transition `{}` destination lies outside region `{}`",
                transition.key, to_region.key
            ),
            BlueprintDiagnosticFocus {
                floor_id: Some(to_floor.floor_id),
                transition_key: Some(transition.key.clone()),
                ..Default::default()
            },
        );
    }
    if (transition.to_local_position[1] - to_floor.elevation_meters).abs()
        > FLOOR_ELEVATION_TOLERANCE_METERS
    {
        push_warning(
            diagnostics,
            "transition_destination_elevation_mismatch",
            format!(
                "transition `{}` destination Y does not match floor {} elevation",
                transition.key, to_floor.floor_id
            ),
            Some(BlueprintDiagnosticFocus {
                floor_id: Some(to_floor.floor_id),
                transition_key: Some(transition.key.clone()),
                ..Default::default()
            }),
        );
    }
}

fn validate_connection_inspection(
    connection: &NavigationRegionConnectionDefinition,
    blueprint: &BuildingNavigationBlueprint,
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
) {
    let focus = BlueprintDiagnosticFocus {
        connection_key: Some(connection.key.clone()),
        ..Default::default()
    };
    let Some(floor) = blueprint.floor_by_key(&connection.floor_key) else {
        push_error(
            diagnostics,
            "connection_floor_missing",
            format!(
                "connection `{}` references missing floor `{}`",
                connection.key, connection.floor_key
            ),
            focus,
        );
        return;
    };
    let from_region = floor.region_by_key(&connection.from_region_key);
    let to_region = floor.region_by_key(&connection.to_region_key);
    if from_region.is_none() || to_region.is_none() {
        push_error(
            diagnostics,
            "connection_region_missing",
            format!(
                "connection `{}` references missing region on floor {}",
                connection.key, floor.floor_id
            ),
            focus,
        );
        return;
    }
    if connection.from_region_key == connection.to_region_key {
        push_error(
            diagnostics,
            "connection_same_region",
            format!(
                "connection `{}` references the same region twice",
                connection.key
            ),
            focus.clone(),
        );
    }
    if connection.kind == NavigationRegionConnectionKind::OpenArch && connection.door_key.is_some()
    {
        push_error(
            diagnostics,
            "connection_door_on_open_arch",
            format!(
                "open-arch connection `{}` cannot have a door key",
                connection.key
            ),
            focus.clone(),
        );
    }
    if !(connection.radius_meters > 0.0)
        || !connection.radius_meters.is_finite()
        || connection.radius_meters < MIN_CONNECTION_RADIUS
    {
        push_error(
            diagnostics,
            "invalid_connection_radius",
            format!("connection `{}` has invalid radius", connection.key),
            focus.clone(),
        );
    } else if connection.radius_meters < TYPICAL_AGENT_RADIUS {
        push_warning(
            diagnostics,
            "connection_radius_below_agent",
            format!(
                "connection `{}` radius is below typical agent radius",
                connection.key
            ),
            Some(focus.clone()),
        );
    }
    if !connection.bidirectional {
        push_info(
            diagnostics,
            "connection_not_bidirectional",
            format!("connection `{}` is one-way", connection.key),
            Some(focus.clone()),
        );
    }
    if connection.door_key.is_some() {
        push_warning(
            diagnostics,
            "connection_door_key_unknown",
            format!(
                "connection `{}` door key is not validated against interior profile here",
                connection.key
            ),
            Some(focus.clone()),
        );
    }
    let from_region = from_region.expect("checked");
    let to_region = to_region.expect("checked");
    let from_pos = Vec2::new(
        connection.from_local_position_xz[0],
        connection.from_local_position_xz[1],
    );
    let to_pos = Vec2::new(
        connection.to_local_position_xz[0],
        connection.to_local_position_xz[1],
    );
    validate_connection_endpoint(
        diagnostics,
        connection,
        floor,
        from_region,
        from_pos,
        "source",
        &connection.from_region_key,
    );
    validate_connection_endpoint(
        diagnostics,
        connection,
        floor,
        to_region,
        to_pos,
        "destination",
        &connection.to_region_key,
    );
    if from_pos.distance(to_pos) > CONNECTION_ENDPOINT_MAX_DISTANCE {
        push_warning(
            diagnostics,
            "connection_endpoints_far_apart",
            format!("connection `{}` endpoints are far apart", connection.key),
            Some(focus),
        );
    }
}

fn validate_connection_endpoint(
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
    connection: &NavigationRegionConnectionDefinition,
    floor: &NavigationFloorDefinition,
    region: &super::definition::NavigationRegionDefinition,
    point: Vec2,
    endpoint: &'static str,
    region_key: &str,
) {
    if !point_inside_polygon(&region.walkable_outline.vertices_xz, point) {
        push_error(
            diagnostics,
            "connection_endpoint_outside_region",
            format!(
                "connection `{}` {endpoint} lies outside region `{region_key}`",
                connection.key
            ),
            BlueprintDiagnosticFocus {
                floor_id: Some(floor.floor_id),
                connection_key: Some(connection.key.clone()),
                ..Default::default()
            },
        );
        return;
    }
    if point_near_boundary(&region.walkable_outline.vertices_xz, point) {
        push_warning(
            diagnostics,
            "connection_endpoint_near_boundary",
            format!(
                "connection `{}` {endpoint} is near region `{region_key}` boundary",
                connection.key
            ),
            Some(BlueprintDiagnosticFocus {
                floor_id: Some(floor.floor_id),
                connection_key: Some(connection.key.clone()),
                ..Default::default()
            }),
        );
    }
    for other in &floor.regions {
        if other.key == region.key {
            continue;
        }
        if point_inside_polygon(&other.walkable_outline.vertices_xz, point) {
            push_error(
                diagnostics,
                "connection_endpoint_in_other_region",
                format!(
                    "connection `{}` {endpoint} lies inside unrelated region `{}`",
                    connection.key, other.key
                ),
                BlueprintDiagnosticFocus {
                    floor_id: Some(floor.floor_id),
                    connection_key: Some(connection.key.clone()),
                    ..Default::default()
                },
            );
        }
    }
}

fn regions_overlap(a: &[[f32; 2]], b: &[[f32; 2]]) -> bool {
    for [x, z] in a {
        if point_strictly_inside_polygon(b, Vec2::new(*x, *z)) {
            return true;
        }
    }
    for [x, z] in b {
        if point_strictly_inside_polygon(a, Vec2::new(*x, *z)) {
            return true;
        }
    }
    for i in 0..a.len() {
        let p = edge_midpoint(a, i);
        if point_strictly_inside_polygon(a, p) && point_strictly_inside_polygon(b, p) {
            return true;
        }
    }
    for i in 0..b.len() {
        let p = edge_midpoint(b, i);
        if point_strictly_inside_polygon(a, p) && point_strictly_inside_polygon(b, p) {
            return true;
        }
    }
    let centroid_a = polygon_centroid(a);
    let centroid_b = polygon_centroid(b);
    if point_strictly_inside_polygon(a, centroid_b) || point_strictly_inside_polygon(b, centroid_a)
    {
        return true;
    }
    false
}

fn polygon_centroid(vertices: &[[f32; 2]]) -> Vec2 {
    if vertices.is_empty() {
        return Vec2::ZERO;
    }
    let mut sum = Vec2::ZERO;
    for [x, z] in vertices {
        sum += Vec2::new(*x, *z);
    }
    sum / vertices.len() as f32
}

fn edge_midpoint(vertices: &[[f32; 2]], index: usize) -> Vec2 {
    let [ax, az] = vertices[index];
    let [bx, bz] = vertices[(index + 1) % vertices.len()];
    Vec2::new((ax + bx) * 0.5, (az + bz) * 0.5)
}

fn point_strictly_inside_polygon(vertices: &[[f32; 2]], point: Vec2) -> bool {
    point_inside_polygon(vertices, point)
        && !point_on_polygon_boundary(vertices, point, REGION_TOUCH_EPSILON)
}

fn region_contains(outer: &[[f32; 2]], inner: &[[f32; 2]]) -> bool {
    inner
        .iter()
        .all(|[x, z]| point_inside_polygon(outer, Vec2::new(*x, *z)))
}

fn regions_touching(a: &[[f32; 2]], b: &[[f32; 2]]) -> bool {
    for i in 0..a.len() {
        let p = Vec2::new(a[i][0], a[i][1]);
        if point_on_polygon_boundary(b, p, REGION_TOUCH_EPSILON) || point_inside_polygon(b, p) {
            return true;
        }
    }
    for i in 0..b.len() {
        let p = Vec2::new(b[i][0], b[i][1]);
        if point_on_polygon_boundary(a, p, REGION_TOUCH_EPSILON) || point_inside_polygon(a, p) {
            return true;
        }
    }
    false
}

fn point_near_boundary(vertices: &[[f32; 2]], point: Vec2) -> bool {
    let n = vertices.len();
    if n < 2 {
        return false;
    }
    for i in 0..n {
        let a = Vec2::new(vertices[i][0], vertices[i][1]);
        let b = Vec2::new(vertices[(i + 1) % n][0], vertices[(i + 1) % n][1]);
        if distance_point_to_segment(point, a, b) <= REGION_ENDPOINT_INSET_MIN {
            return true;
        }
    }
    false
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

fn push_info(
    diagnostics: &mut Vec<BlueprintValidationDiagnostic>,
    code: &'static str,
    message: String,
    focus: Option<BlueprintDiagnosticFocus>,
) {
    diagnostics.push(BlueprintValidationDiagnostic {
        level: BlueprintDiagnosticLevel::Info,
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
        NavigationPolygon2d, NavigationRegionConnectionDefinition, NavigationRegionConnectionKind,
        NavigationRegionDefinition, single_region_floor,
    };

    #[test]
    fn self_intersecting_region_reports_error() {
        let blueprint = BuildingNavigationBlueprint::new("bad", "Bad")
            .with_floors(vec![NavigationFloorDefinition {
                floor_id: 0,
                key: "floor_0".to_string(),
                display_label: "F0".to_string(),
                elevation_meters: 0.0,
                visibility_group_id: 1,
                room_tag: None,
                walkable_outline_legacy: None,
                regions: vec![NavigationRegionDefinition {
                    key: "main".to_string(),
                    display_label: "Main".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d {
                        vertices_xz: vec![[0.0, 0.0], [4.0, 4.0], [4.0, 0.0], [0.0, 4.0]],
                    },
                }],
            }])
            .with_entrances(vec![NavigationEntranceDefinition {
                key: "door".to_string(),
                floor_key: "floor_0".to_string(),
                region_key: Some("main".to_string()),
                local_position_xz: [2.0, 0.0],
                radius_meters: 1.0,
                interior_spawn_local: [2.0, 0.0, 1.0],
                bidirectional: true,
                door_key: None,
            }]);
        let report = validate_blueprint_for_inspection(&blueprint);
        assert!(!report.valid());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "region_self_intersection")
        );
    }

    #[test]
    fn overlapping_regions_produce_error() {
        let mut floor = single_region_floor(
            0,
            "ground",
            "Ground",
            0.0,
            1,
            None,
            NavigationPolygon2d::rectangle(6.0, 4.0),
        );
        floor.regions.push(NavigationRegionDefinition {
            key: "overlap".to_string(),
            display_label: "Overlap".to_string(),
            room_tag: None,
            walkable_outline: NavigationPolygon2d {
                vertices_xz: vec![[3.0, 0.0], [7.0, 0.0], [7.0, 4.0], [3.0, 4.0]],
            },
        });
        let blueprint =
            BuildingNavigationBlueprint::new("overlap", "Overlap").with_floors(vec![floor]);
        let report = validate_blueprint_for_inspection(&blueprint);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "region_overlap")
        );
    }

    #[test]
    fn touching_regions_produce_info() {
        let floor = NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![
                NavigationRegionDefinition {
                    key: "west".to_string(),
                    display_label: "West".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d::rectangle(5.0, 4.0),
                },
                NavigationRegionDefinition {
                    key: "east".to_string(),
                    display_label: "East".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d {
                        vertices_xz: vec![[5.0, 0.0], [10.0, 0.0], [10.0, 4.0], [5.0, 4.0]],
                    },
                },
            ],
        };
        let blueprint = BuildingNavigationBlueprint::new("touch", "Touch").with_floors(vec![floor]);
        let report = validate_blueprint_for_inspection(&blueprint);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "region_touching")
        );
    }

    #[test]
    fn connection_endpoint_in_other_region_is_error() {
        let floor = NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![
                NavigationRegionDefinition {
                    key: "west".to_string(),
                    display_label: "West".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d::rectangle(5.0, 4.0),
                },
                NavigationRegionDefinition {
                    key: "east".to_string(),
                    display_label: "East".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d {
                        vertices_xz: vec![[4.5, 0.0], [10.0, 0.0], [10.0, 4.0], [4.5, 4.0]],
                    },
                },
            ],
        };
        let blueprint = BuildingNavigationBlueprint::new("bad_conn", "Bad")
            .with_floors(vec![floor])
            .with_region_connections(vec![NavigationRegionConnectionDefinition {
                key: "door".to_string(),
                kind: NavigationRegionConnectionKind::Doorway,
                floor_key: "ground".to_string(),
                from_region_key: "west".to_string(),
                to_region_key: "east".to_string(),
                from_local_position_xz: [4.75, 2.0],
                to_local_position_xz: [5.25, 2.0],
                radius_meters: 0.8,
                bidirectional: true,
                enabled: true,
                door_key: None,
            }]);
        let report = validate_blueprint_for_inspection(&blueprint);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "connection_endpoint_in_other_region")
        );
    }
}
