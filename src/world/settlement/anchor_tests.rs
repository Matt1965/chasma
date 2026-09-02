//! Settlement anchor and creation tests (ADR-133 Phase 1).

use bevy::prelude::Vec3;

use super::{
    DEFAULT_TOWN_BOUNDARY_RADIUS_METERS, SettlementCreationError, SettlementKind,
    SettlementOwnership, create_settlement,
};
use crate::world::{
    ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition, WorldData, WorldPosition,
};

fn test_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn position(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

#[test]
fn create_settlement_succeeds_without_building() {
    let mut world = test_world();
    let report = create_settlement(
        &mut world,
        position(10.0, 10.0),
        "Anchor Only",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap();
    let settlement = world
        .settlement_store()
        .get_settlement(report.settlement_id)
        .unwrap();
    assert_eq!(settlement.anchor_id, report.anchor_id);
    assert_eq!(settlement.center, position(10.0, 10.0));
    assert_eq!(
        settlement.boundary_radius_meters,
        DEFAULT_TOWN_BOUNDARY_RADIUS_METERS
    );
    assert!(
        world
            .settlement_anchor_store()
            .get(report.anchor_id)
            .is_some()
    );
}

#[test]
fn create_settlement_creates_exactly_one_anchor() {
    let mut world = test_world();
    let before = world.settlement_anchor_store().sorted_anchor_ids().len();
    let report = create_settlement(
        &mut world,
        position(0.0, 0.0),
        "One Anchor",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap();
    assert_eq!(
        world.settlement_anchor_store().sorted_anchor_ids().len(),
        before + 1
    );
    let anchor = world
        .settlement_anchor_store()
        .get(report.anchor_id)
        .unwrap();
    assert_eq!(anchor.settlement_id, report.settlement_id);
}

#[test]
fn settlement_record_references_anchor_center_and_radius() {
    let mut world = test_world();
    let report = create_settlement(
        &mut world,
        position(4.0, 6.0),
        "Spatial",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        Some(42.0),
        None,
        0,
    )
    .unwrap();
    let settlement = world
        .settlement_store()
        .get_settlement(report.settlement_id)
        .unwrap();
    let anchor = world
        .settlement_anchor_store()
        .get(report.anchor_id)
        .unwrap();
    assert_eq!(settlement.anchor_id, anchor.id);
    assert_eq!(settlement.center, anchor.position);
    assert!((settlement.boundary_radius_meters - 42.0).abs() < f32::EPSILON);
}

#[test]
fn overlapping_settlement_is_rejected() {
    let mut world = test_world();
    create_settlement(
        &mut world,
        position(0.0, 0.0),
        "First",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap();
    let err = create_settlement(
        &mut world,
        position(1.0, 0.0),
        "Second",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        1,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        SettlementCreationError::OverlapsExisting { .. }
    ));
}

#[test]
fn distant_settlement_succeeds() {
    let mut world = test_world();
    create_settlement(
        &mut world,
        position(0.0, 0.0),
        "First",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap();
    create_settlement(
        &mut world,
        position(200.0, 200.0),
        "Second",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        1,
    )
    .unwrap();
    assert_eq!(world.settlement_store().sorted_settlement_ids().len(), 2);
}

#[test]
fn anchor_store_round_trip_preserves_identity() {
    let mut world = test_world();
    let report = create_settlement(
        &mut world,
        position(3.0, 7.0),
        "Roundtrip",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        Some(50.0),
        None,
        2,
    )
    .unwrap();
    let anchors: Vec<_> = world
        .settlement_anchor_store()
        .sorted_anchor_ids()
        .into_iter()
        .filter_map(|id| world.settlement_anchor_store().get(id).cloned())
        .collect();
    let settlements: Vec<_> = world
        .settlement_store()
        .sorted_settlement_ids()
        .into_iter()
        .filter_map(|id| world.settlement_store().get_settlement(id).cloned())
        .collect();
    let treasuries: Vec<_> = world
        .settlement_store()
        .sorted_treasury_ids()
        .into_iter()
        .filter_map(|id| world.settlement_store().get_treasury(id).cloned())
        .collect();

    world.settlement_anchor_store_mut().clear();
    world.settlement_store_mut().clear();
    world
        .settlement_anchor_store_mut()
        .restore_snapshot(anchors, report.anchor_id.raw() + 1)
        .unwrap();
    world
        .settlement_store_mut()
        .restore_snapshot(
            settlements,
            treasuries,
            report.settlement_id.raw() + 1,
            report.treasury_id.raw() + 1,
        )
        .unwrap();

    let settlement = world
        .settlement_store()
        .get_settlement(report.settlement_id)
        .unwrap();
    assert_eq!(settlement.anchor_id, report.anchor_id);
    assert_eq!(settlement.center, position(3.0, 7.0));
    assert!((settlement.boundary_radius_meters - 50.0).abs() < f32::EPSILON);
}
