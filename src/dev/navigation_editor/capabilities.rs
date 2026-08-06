//! Selection-to-action capability matrix for the Navigation Editor (IN-10a).

use crate::dev::inspector::{BlueprintEditSelection, BlueprintInspectionState};
use crate::world::{BuildingNavigationBlueprint, format_region_deletion_error, region_references};

const MIN_VERTEX_COUNT: usize = 3;

/// Resolved visibility, labels, and enablement for contextual editor actions.
#[derive(Debug, Clone, PartialEq)]
pub struct NavEditorSelectionCapabilities {
    pub delete_visible: bool,
    pub delete_label: &'static str,
    pub delete_enabled: bool,
    pub delete_reason: Option<String>,
    pub radius_visible: bool,
    pub radius_meters: Option<f32>,
    pub radius_decrement_enabled: bool,
    pub detail_lines: Vec<String>,
    pub guidance: Option<String>,
}

impl Default for NavEditorSelectionCapabilities {
    fn default() -> Self {
        Self {
            delete_visible: false,
            delete_label: "Delete",
            delete_enabled: false,
            delete_reason: None,
            radius_visible: false,
            radius_meters: None,
            radius_decrement_enabled: true,
            detail_lines: Vec::new(),
            guidance: None,
        }
    }
}

impl NavEditorSelectionCapabilities {
    fn with_guidance(message: impl Into<String>) -> Self {
        Self {
            guidance: Some(message.into()),
            ..Self::default()
        }
    }
}

/// Single source of truth for which contextual actions apply to the current selection.
pub fn navigation_editor_capabilities(
    inspection: &BlueprintInspectionState,
) -> NavEditorSelectionCapabilities {
    let Some(blueprint) = inspection.working_copy.as_ref() else {
        return NavEditorSelectionCapabilities::with_guidance("No working copy loaded.");
    };

    match &inspection.selection {
        BlueprintEditSelection::None => NavEditorSelectionCapabilities::with_guidance(
            "Select a region, vertex, entrance, or connection.",
        ),
        BlueprintEditSelection::Region {
            floor_id,
            region_key,
        } => region_capabilities(blueprint, *floor_id, region_key),
        BlueprintEditSelection::Vertex {
            floor_id,
            region_key,
            index,
        } => vertex_capabilities(blueprint, *floor_id, region_key, *index),
        BlueprintEditSelection::Edge {
            floor_id,
            region_key,
            index,
        } => edge_capabilities(blueprint, *floor_id, region_key, *index),
        BlueprintEditSelection::Entrance { key } => entrance_capabilities(blueprint, key),
        BlueprintEditSelection::Transition { key }
        | BlueprintEditSelection::TransitionTo { key } => transition_capabilities(blueprint, key),
        BlueprintEditSelection::Connection { key }
        | BlueprintEditSelection::ConnectionFrom { key }
        | BlueprintEditSelection::ConnectionTo { key } => connection_capabilities(blueprint, key),
    }
}

fn region_capabilities(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    region_key: &str,
) -> NavEditorSelectionCapabilities {
    let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == floor_id) else {
        return NavEditorSelectionCapabilities::with_guidance("Floor not found.");
    };
    let Some(region) = floor.region_by_key(region_key) else {
        return NavEditorSelectionCapabilities::with_guidance("Region not found.");
    };
    let vertex_count = region.walkable_outline.vertices_xz.len();
    let mut caps = NavEditorSelectionCapabilities {
        delete_visible: true,
        delete_label: "Delete Region",
        detail_lines: vec![
            "REGION".into(),
            format!("Label: {}", region.display_label),
            format!("Key: {region_key}"),
            format!("Floor: {floor_id}"),
            format!("Vertices: {vertex_count}"),
        ],
        ..Default::default()
    };
    if floor.regions.len() <= 1 {
        caps.delete_enabled = false;
        caps.delete_reason = Some("Floor must keep at least one region.".into());
        return caps;
    }
    let references = region_references(blueprint, &floor.key, region_key);
    if !references.is_empty() {
        caps.delete_enabled = false;
        caps.delete_reason = Some(format_region_deletion_error(region_key, &references));
        caps.detail_lines.push(format!(
            "References: {}",
            references
                .iter()
                .map(|r| format!("{} {}", r.feature_kind, r.feature_key))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    } else {
        caps.delete_enabled = true;
    }
    caps
}

fn vertex_capabilities(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    region_key: &str,
    index: usize,
) -> NavEditorSelectionCapabilities {
    let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == floor_id) else {
        return NavEditorSelectionCapabilities::with_guidance("Floor not found.");
    };
    let Some(region) = floor.region_by_key(region_key) else {
        return NavEditorSelectionCapabilities::with_guidance("Region not found.");
    };
    let vertices = &region.walkable_outline.vertices_xz;
    if index >= vertices.len() {
        return NavEditorSelectionCapabilities::with_guidance("Vertex not found.");
    }
    let [x, z] = vertices[index];
    let mut caps = NavEditorSelectionCapabilities {
        delete_visible: true,
        delete_label: "Delete Vertex",
        detail_lines: vec![
            "VERTEX".into(),
            format!("Region: {region_key}"),
            format!("Floor: {floor_id}"),
            format!("Index: {index}"),
            format!("Local X: {x:.2}"),
            format!("Local Z: {z:.2}"),
        ],
        ..Default::default()
    };
    if vertices.len() <= MIN_VERTEX_COUNT {
        caps.delete_enabled = false;
        caps.delete_reason = Some("Polygon must keep at least three vertices.".into());
    } else {
        caps.delete_enabled = true;
    }
    caps
}

fn edge_capabilities(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    region_key: &str,
    index: usize,
) -> NavEditorSelectionCapabilities {
    let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == floor_id) else {
        return NavEditorSelectionCapabilities::with_guidance("Floor not found.");
    };
    let Some(region) = floor.region_by_key(region_key) else {
        return NavEditorSelectionCapabilities::with_guidance("Region not found.");
    };
    let _count = region.walkable_outline.vertices_xz.len();
    NavEditorSelectionCapabilities {
        detail_lines: vec![
            "EDGE".into(),
            format!("Region: {region_key}"),
            format!("Floor: {floor_id}"),
            format!("Edge index: {index}"),
        ],
        guidance: Some("Polygon edges are removed by deleting an adjacent vertex.".into()),
        ..Default::default()
    }
}

fn entrance_capabilities(
    blueprint: &BuildingNavigationBlueprint,
    key: &str,
) -> NavEditorSelectionCapabilities {
    let Some(entrance) = blueprint.entrances.iter().find(|e| e.key == key) else {
        return NavEditorSelectionCapabilities::with_guidance("Entrance not found.");
    };
    let radius = entrance.radius_meters;
    NavEditorSelectionCapabilities {
        delete_visible: true,
        delete_label: "Delete Entrance",
        delete_enabled: true,
        radius_visible: true,
        radius_meters: Some(radius),
        radius_decrement_enabled: radius > 0.25,
        detail_lines: vec![
            "ENTRANCE".into(),
            format!("Key: {key}"),
            format!("Floor: {}", entrance.floor_key),
            format!(
                "Target region: {}",
                entrance.region_key.as_deref().unwrap_or("-")
            ),
            format!("Radius: {radius:.2} m"),
        ],
        ..Default::default()
    }
}

fn transition_capabilities(
    blueprint: &BuildingNavigationBlueprint,
    key: &str,
) -> NavEditorSelectionCapabilities {
    let Some(transition) = blueprint.vertical_transitions.iter().find(|t| t.key == key) else {
        return NavEditorSelectionCapabilities::with_guidance("Transition not found.");
    };
    let radius = transition.from_radius_meters;
    NavEditorSelectionCapabilities {
        delete_visible: true,
        delete_label: "Delete Transition",
        delete_enabled: true,
        radius_visible: true,
        radius_meters: Some(radius),
        radius_decrement_enabled: radius > 0.25,
        detail_lines: vec![
            "VERTICAL TRANSITION".into(),
            format!("Key: {key}"),
            format!(
                "From: {} / {}",
                transition.from_floor_key,
                transition.from_region_key.as_deref().unwrap_or("-")
            ),
            format!(
                "To: {} / {}",
                transition.to_floor_key,
                transition.to_region_key.as_deref().unwrap_or("-")
            ),
            format!("Radius: {radius:.2} m"),
        ],
        ..Default::default()
    }
}

fn connection_capabilities(
    blueprint: &BuildingNavigationBlueprint,
    key: &str,
) -> NavEditorSelectionCapabilities {
    let Some(connection) = blueprint.region_connections.iter().find(|c| c.key == key) else {
        return NavEditorSelectionCapabilities::with_guidance("Connection not found.");
    };
    let radius = connection.radius_meters;
    NavEditorSelectionCapabilities {
        delete_visible: true,
        delete_label: "Delete Connection",
        delete_enabled: true,
        radius_visible: true,
        radius_meters: Some(radius),
        radius_decrement_enabled: radius > 0.25,
        detail_lines: vec![
            "CONNECTION".into(),
            format!("Key: {key}"),
            format!("From: {}", connection.from_region_key),
            format!("To: {}", connection.to_region_key),
            format!("Kind: {:?}", connection.kind),
            format!("Radius: {radius:.2} m"),
            format!(
                "Door key: {}",
                connection.door_key.as_deref().unwrap_or("-")
            ),
            format!("Enabled: {}", connection.enabled),
            format!("Bidirectional: {}", connection.bidirectional),
        ],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::inspector::BlueprintInspectionState;
    use crate::world::{
        BuildingNavigationBlueprint, NavigationFloorDefinition, NavigationPolygon2d,
        NavigationRegionDefinition,
    };

    fn sample_blueprint() -> BuildingNavigationBlueprint {
        BuildingNavigationBlueprint::new("test_nav", "Test").with_floors(vec![
            NavigationFloorDefinition {
                floor_id: 0,
                key: "floor_0".to_string(),
                display_label: "Floor 0".to_string(),
                elevation_meters: 0.0,
                visibility_group_id: 0,
                room_tag: None,
                walkable_outline_legacy: None,
                regions: vec![NavigationRegionDefinition {
                    key: "room_a".to_string(),
                    display_label: "Room A".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d {
                        vertices_xz: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
                    },
                }],
            },
        ])
    }

    fn inspection_with_selection(selection: BlueprintEditSelection) -> BlueprintInspectionState {
        let mut inspection = BlueprintInspectionState::default();
        inspection.working_copy = Some(sample_blueprint());
        inspection.selection = selection;
        inspection
    }

    #[test]
    fn no_selection_hides_delete_and_radius() {
        let caps = navigation_editor_capabilities(&BlueprintInspectionState::default());
        assert!(!caps.delete_visible);
        assert!(!caps.radius_visible);
        assert!(caps.guidance.is_some());
    }

    #[test]
    fn vertex_selection_shows_delete_vertex_not_radius() {
        let inspection = inspection_with_selection(BlueprintEditSelection::Vertex {
            floor_id: 0,
            region_key: "room_a".into(),
            index: 0,
        });
        let caps = navigation_editor_capabilities(&inspection);
        assert!(caps.delete_visible);
        assert_eq!(caps.delete_label, "Delete Vertex");
        assert!(caps.delete_enabled);
        assert!(!caps.radius_visible);
    }

    #[test]
    fn connection_selection_shows_delete_and_radius() {
        let mut inspection = BlueprintInspectionState::default();
        let blueprint = crate::world::two_room_hut_navigation_blueprint();
        let key = blueprint
            .region_connections
            .first()
            .map(|c| c.key.clone())
            .expect("fixture connection");
        inspection.working_copy = Some(blueprint);
        inspection.selection = BlueprintEditSelection::Connection { key };
        let caps = navigation_editor_capabilities(&inspection);
        assert!(caps.delete_visible);
        assert!(caps.radius_visible);
        assert!(caps.radius_meters.is_some());
    }

    #[test]
    fn edge_selection_explains_vertex_deletion() {
        let inspection = inspection_with_selection(BlueprintEditSelection::Edge {
            floor_id: 0,
            region_key: "room_a".into(),
            index: 0,
        });
        let caps = navigation_editor_capabilities(&inspection);
        assert!(!caps.delete_visible);
        assert!(!caps.radius_visible);
        assert!(caps.guidance.is_some());
    }
}
