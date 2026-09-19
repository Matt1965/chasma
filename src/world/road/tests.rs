use std::collections::BTreeMap;

use bevy::prelude::Vec2;

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
