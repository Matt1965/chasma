use std::collections::BTreeMap;

use bevy::prelude::Vec2;

use super::connectivity::move_endpoint_junction_group;
use super::style::default_style_table;
use super::spline::sample_road_spline_at_t;
use super::*;

fn sample_two_point_road() -> Road {
    Road {
        id: RoadId::new("road_a"),
        display_name: String::new(),
        style: RoadStyleId::DirtRoad,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![RoadControlPoint::new(0.0, 0.0), RoadControlPoint::new(10.0, 0.0)],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    }
}

#[test]
fn centripetal_catmull_rom_passes_through_control_points() {
    let road = Road {
        id: RoadId::new("curve"),
        display_name: String::new(),
        style: RoadStyleId::DirtRoad,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![
            RoadControlPoint::new(0.0, 0.0),
            RoadControlPoint::new(10.0, 0.0),
            RoadControlPoint::new(20.0, 10.0),
            RoadControlPoint::new(30.0, 10.0),
        ],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    };

    let start = sample_road_spline_at_distance(&road, 0.0).expect("start sample");
    let end = sample_road_spline_at_distance(
        &road,
        sample_road_polyline(&road, 1.0)
            .last()
            .expect("polyline")
            .distance_m,
    )
    .expect("end sample");

    assert!((start.position - Vec2::new(0.0, 0.0)).length() < 1e-3);
    assert!((end.position - Vec2::new(30.0, 10.0)).length() < 1e-3);
}

#[test]
fn centripetal_sampling_is_deterministic() {
    let road = sample_two_point_road();
    let first = sample_road_polyline(&road, 2.0);
    let second = sample_road_polyline(&road, 2.0);
    assert_eq!(first, second);
}

#[test]
fn uneven_spacing_does_not_produce_nan_samples() {
    let road = Road {
        id: RoadId::new("uneven"),
        display_name: String::new(),
        style: RoadStyleId::DirtRoad,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![
            RoadControlPoint::new(0.0, 0.0),
            RoadControlPoint::new(1.0, 0.0),
            RoadControlPoint::new(30.0, 0.0),
            RoadControlPoint::new(31.0, 0.0),
        ],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    };

    for sample in sample_road_polyline(&road, 1.0) {
        assert!(sample.position.is_finite());
        assert!(sample.tangent.is_finite());
    }
}

#[test]
fn minimum_valid_road_samples_linearly() {
    let road = sample_two_point_road();
    validate_road_network(&RoadNetwork {
        version: ROAD_NETWORK_SCHEMA_VERSION,
        styles: default_style_table(),
        roads: BTreeMap::from([(road.id.clone(), road.clone())]),
        junctions: BTreeMap::new(),
        crossings: Vec::new(),
    })
    .expect("valid road");

    let midpoint = sample_road_spline_at_t(&road, 0.5).expect("midpoint");
    assert!((midpoint.position - Vec2::new(5.0, 0.0)).length() < 1e-3);
}

#[test]
fn invalid_point_count_is_rejected() {
    let road = Road {
        id: RoadId::new("too_short"),
        display_name: String::new(),
        style: RoadStyleId::DirtRoad,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![RoadControlPoint::new(0.0, 0.0)],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    };

    let error = road.validate().unwrap_err();
    assert!(matches!(
        error,
        RoadError::InvalidControlPointCount { .. }
    ));
}

#[test]
fn nan_coordinates_are_rejected() {
    let road = Road {
        id: RoadId::new("nan"),
        display_name: String::new(),
        style: RoadStyleId::DirtRoad,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![
            RoadControlPoint::new(f32::NAN, 0.0),
            RoadControlPoint::new(1.0, 0.0),
        ],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    };

    let error = road.validate().unwrap_err();
    assert!(matches!(error, RoadError::InvalidControlPoint(_)));
}

#[test]
fn ron_round_trip_is_stable() {
    let network = RoadNetwork {
        version: ROAD_NETWORK_SCHEMA_VERSION,
        styles: default_style_table(),
        roads: BTreeMap::from([(RoadId::new("road_a"), sample_two_point_road())]),
        junctions: BTreeMap::new(),
        crossings: Vec::new(),
    };

    let serialized = serialize_road_network_ron(&network).expect("serialize");
    let parsed = parse_road_network_ron(&serialized).expect("parse");
    assert_eq!(parsed, network);
}

#[test]
fn missing_file_yields_empty_network() {
    let path = std::env::temp_dir().join(format!(
        "chasma_missing_road_network_{}.ron",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let network = load_road_network_from_path(&path).expect("missing file load");
    assert!(network.roads.is_empty());
    assert!(network.junctions.is_empty());
}

#[test]
fn duplicate_road_id_is_rejected() {
    let road = sample_two_point_road();
    let mut duplicate_id_road = road.clone();
    duplicate_id_road.control_points[1] = RoadControlPoint::new(20.0, 0.0);
    let network = RoadNetwork {
        version: ROAD_NETWORK_SCHEMA_VERSION,
        styles: default_style_table(),
        roads: BTreeMap::from([
            (RoadId::new("road_a"), road),
            (RoadId::new("road_b"), duplicate_id_road),
        ]),
        junctions: BTreeMap::new(),
        crossings: Vec::new(),
    };

    let error = validate_road_network(&network).unwrap_err();
    assert!(matches!(error, RoadError::DuplicateRoadId(_)));
}

#[test]
fn invalid_style_reference_is_rejected() {
    let mut road = sample_two_point_road();
    road.style = RoadStyleId::Trail;
    let network = RoadNetwork {
        version: ROAD_NETWORK_SCHEMA_VERSION,
        styles: BTreeMap::from([(RoadStyleId::DirtRoad, RoadStyleDefaults::dirt_road())]),
        roads: BTreeMap::from([(road.id.clone(), road)]),
        junctions: BTreeMap::new(),
        crossings: Vec::new(),
    };

    let error = validate_road_network(&network).unwrap_err();
    assert!(matches!(error, RoadError::UnknownStyle(RoadStyleId::Trail)));
}

#[test]
fn junction_reference_validation_requires_mirrored_members() {
    let junction_id = JunctionId::new("junction_a");
    let road = Road {
        id: RoadId::new("road_a"),
        display_name: String::new(),
        style: RoadStyleId::DirtRoad,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![RoadControlPoint::new(0.0, 0.0), RoadControlPoint::new(10.0, 0.0)],
        start_attachment: Some(RoadEndpointAttachment {
            junction_id: junction_id.clone(),
            is_start: true,
        }),
        end_attachment: None,
        tee_attachments: Vec::new(),
    };
    let network = RoadNetwork {
        version: ROAD_NETWORK_SCHEMA_VERSION,
        styles: default_style_table(),
        roads: BTreeMap::from([(road.id.clone(), road)]),
        junctions: BTreeMap::from([(
            junction_id.clone(),
            Junction {
                id: junction_id,
                members: Vec::new(),
            },
        )]),
        crossings: Vec::new(),
    };

    let error = validate_road_network(&network).unwrap_err();
    assert!(matches!(error, RoadError::InvalidJunctionReference(_)));
}

fn two_road_network(
    road_a: Road,
    road_b: Road,
) -> RoadNetwork {
    RoadNetwork {
        version: ROAD_NETWORK_SCHEMA_VERSION,
        styles: default_style_table(),
        roads: BTreeMap::from([(road_a.id.clone(), road_a), (road_b.id.clone(), road_b)]),
        junctions: BTreeMap::new(),
        crossings: Vec::new(),
    }
}

fn horizontal_road(id: &str, y: f32, x0: f32, x1: f32) -> Road {
    Road {
        id: RoadId::new(id),
        display_name: String::new(),
        style: RoadStyleId::DirtRoad,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![
            RoadControlPoint::new(x0, y),
            RoadControlPoint::new(x1, y),
        ],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    }
}

#[test]
fn endpoint_snap_creates_shared_junction() {
    let mut network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 10.0),
        horizontal_road("road_b", 5.0, 10.0, 20.0),
    );
    try_snap_endpoint(&mut network, &RoadId::new("road_a"), false, Vec2::new(10.0, 0.0))
        .expect("snap");
    try_snap_endpoint(&mut network, &RoadId::new("road_b"), true, Vec2::new(10.0, 0.0))
        .expect("snap");
    assert_eq!(network.junctions.len(), 1);
    let road_a = network.roads.get(&RoadId::new("road_a")).expect("road");
    let road_b = network.roads.get(&RoadId::new("road_b")).expect("road");
    assert_eq!(road_a.control_points.last().unwrap().xz(), Vec2::new(10.0, 0.0));
    assert_eq!(road_b.control_points.first().unwrap().xz(), Vec2::new(10.0, 0.0));
    validate_road_network(&network).expect("valid");
}

#[test]
fn third_road_reuses_existing_endpoint_junction() {
    let mut network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 10.0),
        horizontal_road("road_b", 0.0, 10.0, 20.0),
    );
    try_snap_endpoint(&mut network, &RoadId::new("road_a"), false, Vec2::new(10.0, 0.0))
        .expect("snap");
    try_snap_endpoint(&mut network, &RoadId::new("road_b"), true, Vec2::new(10.0, 0.0))
        .expect("snap");
    let junction_id = network.junctions.keys().next().cloned().expect("junction");
    let road_c = horizontal_road("road_c", 0.0, 10.0, 20.0);
    network.roads.insert(road_c.id.clone(), road_c);
    try_snap_endpoint(&mut network, &RoadId::new("road_c"), true, Vec2::new(10.0, 0.0))
        .expect("snap");
    assert_eq!(network.junctions.len(), 1);
    assert_eq!(
        network.junctions.get(&junction_id).unwrap().members.len(),
        3
    );
}

#[test]
fn endpoint_junction_survives_ron_roundtrip() {
    let mut network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 10.0),
        horizontal_road("road_b", 0.0, 10.0, 20.0),
    );
    try_snap_endpoint(&mut network, &RoadId::new("road_a"), false, Vec2::new(10.0, 0.0))
        .expect("snap");
    try_snap_endpoint(&mut network, &RoadId::new("road_b"), true, Vec2::new(10.0, 0.0))
        .expect("snap");
    let serialized = serialize_road_network_ron(&network).expect("serialize");
    let parsed = parse_road_network_ron(&serialized).expect("parse");
    assert_eq!(parsed.junctions.len(), 1);
    validate_road_network(&parsed).expect("valid");
}

#[test]
fn tee_junction_projects_branch_endpoint_onto_host() {
    let mut network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 20.0),
        Road {
            id: RoadId::new("road_b"),
            display_name: String::new(),
            style: RoadStyleId::DirtRoad,
            style_overrides: RoadStyleOverrides::default(),
            control_points: vec![
                RoadControlPoint::new(10.0, 8.0),
                RoadControlPoint::new(10.0, 16.0),
            ],
            start_attachment: None,
            end_attachment: None,
            tee_attachments: Vec::new(),
        },
    );
    try_snap_endpoint(&mut network, &RoadId::new("road_b"), true, Vec2::new(10.0, 0.0))
        .expect("tee snap");
    assert_eq!(network.junctions.len(), 1);
    let junction = network.junctions.values().next().expect("junction");
    let host_member = junction
        .members
        .iter()
        .find(|member| member.role == JunctionMemberRole::TeeHost)
        .expect("host");
    assert_eq!(host_member.road_id.as_str(), "road_a");
    assert!((host_member.host_t.unwrap() - 0.5).abs() < 0.05);
    let branch = network.roads.get(&RoadId::new("road_b")).expect("branch");
    assert_eq!(branch.control_points.first().unwrap().xz(), Vec2::new(10.0, 0.0));
    validate_road_network(&network).expect("valid");
}

#[test]
fn tee_branch_follows_host_shape_edit() {
    let mut network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 20.0),
        Road {
            id: RoadId::new("road_b"),
            display_name: String::new(),
            style: RoadStyleId::DirtRoad,
            style_overrides: RoadStyleOverrides::default(),
            control_points: vec![
                RoadControlPoint::new(10.0, 8.0),
                RoadControlPoint::new(10.0, 16.0),
            ],
            start_attachment: None,
            end_attachment: None,
            tee_attachments: Vec::new(),
        },
    );
    try_snap_endpoint(&mut network, &RoadId::new("road_b"), true, Vec2::new(10.0, 0.0))
        .expect("tee snap");
    let host = network.roads.get_mut(&RoadId::new("road_a")).expect("host");
    host.control_points[1] = RoadControlPoint::new(20.0, 5.0);
    refresh_tee_branches_for_host(&mut network, &RoadId::new("road_a"));
    let branch = network.roads.get(&RoadId::new("road_b")).expect("branch");
    let host = network.roads.get(&RoadId::new("road_a")).expect("host");
    let host_t = host
        .tee_attachments
        .first()
        .expect("tee")
        .host_t;
    let expected = sample_road_spline_at_t(host, host_t)
        .expect("sample")
        .position;
    assert!((branch.control_points.first().unwrap().xz() - expected).length() < 0.2);
}

#[test]
fn derived_crossing_detects_perpendicular_roads() {
    let network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 20.0),
        Road {
            id: RoadId::new("road_b"),
            display_name: String::new(),
            style: RoadStyleId::DirtRoad,
            style_overrides: RoadStyleOverrides::default(),
            control_points: vec![
                RoadControlPoint::new(10.0, -10.0),
                RoadControlPoint::new(10.0, 10.0),
            ],
            start_attachment: None,
            end_attachment: None,
            tee_attachments: Vec::new(),
        },
    );
    let crossings = derive_ground_crossings(&network);
    assert_eq!(crossings.len(), 1);
    assert!(crossings[0].connected);
    assert!((crossings[0].position - Vec2::new(10.0, 0.0)).length() < 1.0);
}

#[test]
fn parallel_roads_do_not_create_derived_crossing() {
    let network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 20.0),
        horizontal_road("road_b", 5.0, 0.0, 20.0),
    );
    assert!(derive_ground_crossings(&network).is_empty());
}

#[test]
fn move_shared_endpoint_junction_moves_all_members() {
    let mut network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 10.0),
        horizontal_road("road_b", 0.0, 10.0, 20.0),
    );
    try_snap_endpoint(&mut network, &RoadId::new("road_a"), false, Vec2::new(10.0, 0.0))
        .expect("snap");
    try_snap_endpoint(&mut network, &RoadId::new("road_b"), true, Vec2::new(10.0, 0.0))
        .expect("snap");
    let junction_id = network.junctions.keys().next().cloned().expect("junction");
    move_endpoint_junction_group(&mut network, &junction_id, Vec2::new(12.0, 2.0))
        .expect("move");
    let road_a = network.roads.get(&RoadId::new("road_a")).expect("road");
    let road_b = network.roads.get(&RoadId::new("road_b")).expect("road");
    assert_eq!(road_a.control_points.last().unwrap().xz(), Vec2::new(12.0, 2.0));
    assert_eq!(road_b.control_points.first().unwrap().xz(), Vec2::new(12.0, 2.0));
}

#[test]
fn delete_attached_road_removes_two_member_junction() {
    let mut network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 10.0),
        horizontal_road("road_b", 0.0, 10.0, 20.0),
    );
    try_snap_endpoint(&mut network, &RoadId::new("road_a"), false, Vec2::new(10.0, 0.0))
        .expect("snap");
    try_snap_endpoint(&mut network, &RoadId::new("road_b"), true, Vec2::new(10.0, 0.0))
        .expect("snap");
    remove_road_and_cleanup_junctions(&mut network, &RoadId::new("road_a"));
    assert!(network.junctions.is_empty());
    validate_road_network(&network).expect("valid");
}

#[test]
fn three_way_junction_survives_single_road_deletion() {
    let mut network = two_road_network(
        horizontal_road("road_a", 0.0, 0.0, 10.0),
        horizontal_road("road_b", 0.0, 10.0, 20.0),
    );
    try_snap_endpoint(&mut network, &RoadId::new("road_a"), false, Vec2::new(10.0, 0.0))
        .expect("snap");
    try_snap_endpoint(&mut network, &RoadId::new("road_b"), true, Vec2::new(10.0, 0.0))
        .expect("snap");
    let road_c = Road {
        id: RoadId::new("road_c"),
        display_name: String::new(),
        style: RoadStyleId::DirtRoad,
        style_overrides: RoadStyleOverrides::default(),
        control_points: vec![
            RoadControlPoint::new(10.0, 0.0),
            RoadControlPoint::new(10.0, 10.0),
        ],
        start_attachment: None,
        end_attachment: None,
        tee_attachments: Vec::new(),
    };
    network.roads.insert(road_c.id.clone(), road_c);
    try_snap_endpoint(&mut network, &RoadId::new("road_c"), true, Vec2::new(10.0, 0.0))
        .expect("snap");
    remove_road_and_cleanup_junctions(&mut network, &RoadId::new("road_c"));
    assert_eq!(network.junctions.len(), 1);
    assert_eq!(network.junctions.values().next().unwrap().members.len(), 2);
    validate_road_network(&network).expect("valid");
}

#[test]
fn invalid_host_t_is_rejected_by_validation() {
    let junction_id = JunctionId::new("junction_a");
    let road_a = horizontal_road("road_a", 0.0, 0.0, 10.0);
    let mut road_b = horizontal_road("road_b", 0.0, 10.0, 0.0);
    road_b.start_attachment = Some(RoadEndpointAttachment {
        junction_id: junction_id.clone(),
        is_start: true,
    });
    let network = RoadNetwork {
        version: ROAD_NETWORK_SCHEMA_VERSION,
        styles: default_style_table(),
        roads: BTreeMap::from([(road_a.id.clone(), road_a), (road_b.id.clone(), road_b)]),
        junctions: BTreeMap::from([(
            junction_id.clone(),
            Junction {
                id: junction_id,
                members: vec![
                    JunctionMember {
                        road_id: RoadId::new("road_a"),
                        role: JunctionMemberRole::TeeHost,
                        host_t: Some(1.5),
                    },
                    JunctionMember {
                        road_id: RoadId::new("road_b"),
                        role: JunctionMemberRole::TeeBranch,
                        host_t: None,
                    },
                ],
            },
        )]),
        crossings: Vec::new(),
    };
    let error = validate_road_network(&network).unwrap_err();
    assert!(matches!(
        error,
        RoadError::InvalidJunctionReference(_)
    ));
}

#[test]
fn junction_duplicate_member_is_rejected() {
    let junction_id = JunctionId::new("junction_a");
    let road_id = RoadId::new("road_a");
    let network = RoadNetwork {
        version: ROAD_NETWORK_SCHEMA_VERSION,
        styles: default_style_table(),
        roads: BTreeMap::from([(road_id.clone(), sample_two_point_road())]),
        junctions: BTreeMap::from([(
            junction_id.clone(),
            Junction {
                id: junction_id,
                members: vec![
                    JunctionMember {
                        road_id: road_id.clone(),
                        role: JunctionMemberRole::EndpointStart,
                        host_t: None,
                    },
                    JunctionMember {
                        road_id: road_id.clone(),
                        role: JunctionMemberRole::EndpointEnd,
                        host_t: None,
                    },
                ],
            },
        )]),
        crossings: Vec::new(),
    };

    let error = validate_road_network(&network).unwrap_err();
    assert!(matches!(
        error,
        RoadError::DuplicateJunctionMember { .. }
    ));
}
