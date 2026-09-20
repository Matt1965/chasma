use bevy::prelude::Vec2;

use crate::world::{
    Road, RoadControlPoint, RoadId, RoadNetwork, RoadStyleId, derive_ground_crossings,
    endpoint_has_attachment, try_snap_endpoint,
};

use super::domain::{
    delete_control_point, finish_create_road, generate_road_id, insert_control_point,
    move_control_point, pick_control_point_at, pick_road_at,
};
use super::state::{RoadEditMode, RoadEditorUiState};

fn sample_network_with_road() -> RoadNetwork {
    let road = Road {
        id: RoadId::new("road_a"),
        display_name: "Main".into(),
        style: RoadStyleId::DirtRoad,
        style_overrides: Default::default(),
        control_points: vec![
            RoadControlPoint::new(0.0, 0.0),
            RoadControlPoint::new(10.0, 0.0),
            RoadControlPoint::new(20.0, 0.0),
        ],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    };
    let mut network = RoadNetwork::empty();
    network.roads.insert(road.id.clone(), road);
    network
}

#[test]
fn begin_create_mode_starts_empty_draft() {
    let mut editor = RoadEditorUiState::default();
    editor.begin_create();
    assert_eq!(editor.mode, RoadEditMode::Create);
    assert!(editor.create_points.is_empty());
}

#[test]
fn finish_create_rejects_single_point() {
    let mut network = RoadNetwork::empty();
    let mut editor = RoadEditorUiState::default();
    editor.begin_create();
    editor.create_points.push(RoadControlPoint::new(0.0, 0.0));
    let result = finish_create_road(
        &mut network,
        editor.create_display_name.clone(),
        editor.create_style,
        editor.create_points.clone(),
    );
    assert!(result.is_err());
}

#[test]
fn finish_create_adds_valid_road() {
    let mut network = RoadNetwork::empty();
    let id = finish_create_road(
        &mut network,
        "Trail".into(),
        RoadStyleId::Trail,
        vec![
            RoadControlPoint::new(0.0, 0.0),
            RoadControlPoint::new(5.0, 0.0),
        ],
    )
    .expect("valid road");
    assert!(network.roads.contains_key(&id));
}

#[test]
fn cancel_clears_draft_state() {
    let mut editor = RoadEditorUiState::default();
    editor.begin_create();
    editor.create_points.push(RoadControlPoint::new(1.0, 2.0));
    editor.cancel_active();
    assert_eq!(editor.mode, RoadEditMode::Inactive);
    assert!(editor.create_points.is_empty());
}

#[test]
fn pick_road_and_control_point() {
    let network = sample_network_with_road();
    let road_id = pick_road_at(&network, Vec2::new(10.0, 0.0)).expect("road");
    assert_eq!(road_id.as_str(), "road_a");
    let road = network.roads.get(&road_id).expect("road");
    let index = pick_control_point_at(road, Vec2::new(10.0, 0.0)).expect("point");
    assert_eq!(index, 1);
}

#[test]
fn move_and_insert_control_points() {
    let mut network = sample_network_with_road();
    let road_id = RoadId::new("road_a");
    let road = network.roads.get_mut(&road_id).expect("road");
    move_control_point(road, 1, Vec2::new(12.0, 1.0)).expect("move");
    assert_eq!(road.control_points[1].x, 12.0);
    insert_control_point(&mut network, &road_id, 1, Vec2::new(15.0, 0.5))
        .expect("insert");
    assert_eq!(network.roads.get(&road_id).unwrap().control_points.len(), 4);
}

#[test]
fn minimum_point_count_is_enforced() {
    let mut network = sample_network_with_road();
    let road = network.roads.get_mut(&RoadId::new("road_a")).expect("road");
    delete_control_point(road, 0).expect("three-point road can delete one");
    let err = delete_control_point(road, 0).unwrap_err();
    assert!(err.contains("at least two"));
}

#[test]
fn generated_road_ids_are_unique() {
    let mut network = sample_network_with_road();
    let first = generate_road_id(&network);
    network.roads.insert(
        first.clone(),
        Road {
            id: first.clone(),
            display_name: String::new(),
            style: RoadStyleId::DirtRoad,
            style_overrides: Default::default(),
            control_points: vec![
                RoadControlPoint::new(0.0, 0.0),
                RoadControlPoint::new(1.0, 0.0),
            ],
            start_attachment: None,
            end_attachment: None,
            tee_attachments: Vec::new(),
        },
    );
    let second = generate_road_id(&network);
    assert_ne!(first, second);
}

#[test]
fn attached_endpoint_delete_is_blocked() {
    let mut network = sample_network_with_road();
    try_snap_endpoint(&mut network, &RoadId::new("road_a"), false, Vec2::new(20.0, 0.0))
        .ok();
    let road = network.roads.get_mut(&RoadId::new("road_a")).expect("road");
    road.end_attachment = Some(crate::world::RoadEndpointAttachment {
        junction_id: crate::world::JunctionId::new("junction_test"),
        is_start: false,
    });
    assert!(endpoint_has_attachment(road, 2));
    let err = delete_control_point(road, 2).unwrap_err();
    assert!(err.contains("detach"));
}

#[test]
fn derived_crossing_recompute_does_not_mark_editor_dirty() {
    let mut editor = RoadEditorUiState::default();
    let network = sample_network_with_road();
    editor.sync_baseline_from(&network);
    let _ = derive_ground_crossings(&network);
    assert!(!editor.dirty);
}

#[test]
fn rename_does_not_change_road_id() {
    let mut network = sample_network_with_road();
    let road = network.roads.get_mut(&RoadId::new("road_a")).expect("road");
    let original_id = road.id.clone();
    road.display_name = "Renamed".into();
    assert_eq!(road.id, original_id);
}
