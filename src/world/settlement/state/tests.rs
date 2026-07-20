//! SettlementState SA1 tests.

use super::*;
use crate::world::settlement::{SettlementId, SettlementOwnership, SettlementRecord, SettlementStore};
use crate::world::{
    Affiliation, BuildingId, ChunkCoord, ChunkLayout, LocalPosition, TreasuryId, WorldData,
    WorldPosition,
};
fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn sample_state(id: SettlementId, player: bool) -> SettlementState {
    SettlementState::new(id, SettlementKind::Town, player)
}

#[test]
fn player_and_ai_share_identical_runtime_structure() {
    let player = sample_state(SettlementId::new(1), true);
    let ai = sample_state(SettlementId::new(2), false);
    assert_eq!(player.kind, ai.kind);
    assert_eq!(
        player.need_targets.len(),
        ai.need_targets.len(),
        "same default targets"
    );
    assert!(player.policies.player_controlled);
    assert!(!ai.policies.player_controlled);
    // Structure fields exist on both — only policy flag differs.
    assert_eq!(
        std::mem::size_of_val(&player),
        std::mem::size_of_val(&ai)
    );
}

#[test]
fn serialize_excludes_dirty_flag_and_restores_dirty() {
    let mut state = sample_state(SettlementId::new(7), false);
    state.planner.dirty = false;
    state.planner.last_evaluation_tick = 120;
    state.planner.next_scheduled_evaluation_tick = 180;
    state.emergencies.starvation = true;
    state.need_targets.push(NeedTarget::new(NeedCategory::Research, 5, 0.2));

    let serialized = ron::ser::to_string(&state).expect("serialize");
    assert!(
        !serialized.contains("dirty"),
        "dirty must not appear in serialized SettlementState: {serialized}"
    );
    assert!(serialized.contains("starvation"));
    assert!(serialized.contains("last_evaluation_tick"));

    let restored: SettlementState = ron::from_str(&serialized).expect("deserialize");
    assert!(
        restored.planner.dirty,
        "rebuild principle: dirty defaults true after deserialize"
    );
    assert_eq!(restored.planner.last_evaluation_tick, 120);
    assert!(restored.emergencies.starvation);
    assert_eq!(restored.settlement_id, SettlementId::new(7));
}

#[test]
fn store_save_roundtrip_marks_dirty_and_preserves_targets() {
    let mut store = SettlementStateStore::default();
    let id = SettlementId::new(3);
    let mut state = sample_state(id, true);
    state.policies.aggression = 200;
    state.planner.dirty = false;
    state.modifiers.push(SettlementModifier {
        source: SettlementModifierSource::Scenario,
        key: "drought".into(),
        magnitude: -0.25,
        expires_tick: Some(999),
    });
    store.insert(state);

    let save = store.export_save_state();
    let mut restored = SettlementStateStore::default();
    restored.import_save_state(save);

    let got = restored.get(id).expect("state");
    assert!(got.planner.dirty);
    assert_eq!(got.policies.aggression, 200);
    assert_eq!(got.modifiers.len(), 1);
    assert_eq!(got.modifiers[0].key, "drought");
}

#[test]
fn validation_catches_orphan_and_duplicate_need() {
    let mut settlement_store = SettlementStore::default();
    let mut state_store = SettlementStateStore::default();

    let orphan = SettlementId::new(99);
    state_store.insert(sample_state(orphan, false));

    let mut bad = sample_state(SettlementId::new(1), false);
    bad.need_targets.push(NeedTarget::new(NeedCategory::Food, 10, 1.0));
    bad.need_targets.push(NeedTarget::new(NeedCategory::Food, 20, 1.0));
    state_store.insert(bad);

    let errors = validate_settlement_states(&settlement_store, &state_store);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SettlementStateValidationError::OrphanState(_)))
    );
    assert!(errors.iter().any(|e| matches!(
        e,
        SettlementStateValidationError::DuplicateNeedCategory { .. }
    )));

    // Insert matching record so MissingState can be tested the other way.
    let id = SettlementId::new(5);
    let _ = settlement_store.insert_settlement(
        SettlementRecord {
            id,
            display_name: "Camp".into(),
            treasury_id: TreasuryId::new(5),
            anchor_building_id: BuildingId::new(1),
            ownership: SettlementOwnership::player_default(),
            interaction_position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(bevy::prelude::Vec3::ZERO),
            ),
            created_tick: 0,
        },
        crate::world::settlement::SettlementTreasuryRecord {
            id: TreasuryId::new(5),
            settlement_id: id,
            ownership: SettlementOwnership::player_default(),
            balance_gold: 0,
            created_tick: 0,
            metadata: String::new(),
        },
    );
    let errors = validate_settlement_states(&settlement_store, &state_store);
    assert!(errors.iter().any(|e| matches!(
        e,
        SettlementStateValidationError::MissingState(missing) if *missing == id
    )));
}

#[test]
fn settlement_state_independent_of_chunk_residency() {
    // SettlementState lives on WorldData, not chunk stores — survives empty chunks.
    let mut world = WorldData::new(layout());
    let id = SettlementId::new(1);
    world.settlement_state_store_mut().insert(sample_state(id, false));
    assert!(world.iter().next().is_none(), "no chunks loaded");
    assert!(world.settlement_state_store().get(id).is_some());
}

#[test]
fn kind_parse_rejects_unknown() {
    assert_eq!(SettlementKind::parse("hive"), Some(SettlementKind::Hive));
    assert!(SettlementKind::parse("spaceship").is_none());
}

#[test]
fn no_transient_analysis_fields_on_state() {
    let state = sample_state(SettlementId::new(1), false);
    let serialized = ron::ser::to_string(&state).expect("serialize");
    for forbidden in [
        "pressures:",
        "priorities:",
        "diagnostics:",
        "response_graph",
        "chosen_producers",
        "last_diagnostics",
        "dirty:",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "transient field `{forbidden}` must not be serialized: {serialized}"
        );
    }
    assert!(serialized.contains("need_targets"));
    assert!(serialized.contains("policies"));
    assert!(serialized.contains("planner"));
    assert!(serialized.contains("emergencies"));
    assert!(serialized.contains("modifiers"));
}

#[test]
fn affiliation_player_default_sets_player_controlled_seam() {
    let ownership = SettlementOwnership {
        owner_id: None,
        team_id: None,
        affiliation: Affiliation::Player,
    };
    assert_eq!(ownership.affiliation, Affiliation::Player);
    let state = SettlementState::new(SettlementId::new(1), SettlementKind::Camp, true);
    assert!(state.policies.player_controlled);
}
