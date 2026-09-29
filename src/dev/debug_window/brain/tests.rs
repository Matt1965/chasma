//! Brain diagnostics model and history tests.

use super::history::{BrainDecisionHistory, record_brain_unit_history};
use super::model::{
    build_settlement_brain_snapshot, build_unit_brain_snapshot, candidate_race_rows,
    intent_arbitration_bars, spine_stages_for_unit, NO_SELECTED_UNIT_MESSAGE,
    NO_SETTLEMENT_FOCUS_MESSAGE,
};
use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::client::CameraSettlementContext;
use crate::units::input::SelectedUnits;
use crate::world::{
    ArbitrationScoreBreakdown, ChunkLayout, CombatState, IntentRejectionReason,
    NeedId, RejectedIntentCandidate, ResponseId, SettlementId, SettlementIntentPlan, TaskId,
    TaskPriority, TaskRecord, TaskTarget, TaskType, UnitId, UnitState, WorkerCandidateDiagnostic,
    WorkerEvaluation, WorldData,
};

fn test_layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

#[test]
fn unit_view_empty_without_selection() {
    let world = WorldData::new(test_layout());
    let snap = build_unit_brain_snapshot(
        &WorldSelectionState::default(),
        &SelectedUnits::default(),
        &world,
        None,
        &[],
    );
    assert_eq!(snap.empty_message.as_deref(), Some(NO_SELECTED_UNIT_MESSAGE));
    assert!(snap.unit_id.is_none());
}

#[test]
fn selection_change_cannot_show_stale_unit_id() {
    let world = WorldData::new(test_layout());
    let mut selection = WorldSelectionState::default();
    let mut selected = SelectedUnits::default();
    selected.set_single(UnitId::new(1));
    selection.category = WorldSelectionCategory::Units;
    let a = build_unit_brain_snapshot(&selection, &selected, &world, None, &[]);
    selected.set_single(UnitId::new(99));
    let b = build_unit_brain_snapshot(&selection, &selected, &world, None, &[]);
    assert!(a.empty_message.is_some());
    assert!(b.empty_message.is_some());
    assert!(a.unit_id.is_none());
    assert!(b.unit_id.is_none());
}

#[test]
fn settlement_view_empty_without_focus() {
    let world = WorldData::new(test_layout());
    let snap = build_settlement_brain_snapshot(&CameraSettlementContext::default(), &world);
    assert_eq!(snap.empty_message.as_deref(), Some(NO_SETTLEMENT_FOCUS_MESSAGE));
}

#[test]
fn sa7_candidate_ordering_uses_runtime_diagnostics() {
    let eval = WorkerEvaluation {
        unit_id: UnitId::new(1),
        chosen_task_id: Some(TaskId::new(2)),
        chosen_score: 900.0,
        candidate_count: 3,
        top_candidates: Vec::new(),
        ranked_candidates: vec![
            WorkerCandidateDiagnostic {
                task_id: Some(TaskId::new(1)),
                task_type: TaskType::OperateWorkstation,
                priority: TaskPriority::Low,
                distance_meters: 10.0,
                priority_component: 300.0,
                distance_component: 0.5,
                total_score: 299.5,
                eligible: true,
                block_reason: None,
                chosen: false,
            },
            WorkerCandidateDiagnostic {
                task_id: Some(TaskId::new(2)),
                task_type: TaskType::OperateWorkstation,
                priority: TaskPriority::High,
                distance_meters: 5.0,
                priority_component: 3000.0,
                distance_component: 0.25,
                total_score: 2999.75,
                eligible: true,
                block_reason: None,
                chosen: true,
            },
        ],
        reservation_point: None,
        idle: false,
        notes: String::new(),
    };
    let (rows, extra, _) = candidate_race_rows(Some(&eval));
    assert_eq!(extra, 0);
    assert!(rows[0].chosen);
    assert!(rows[0].total_score > rows.get(1).map(|r| r.total_score).unwrap_or(0.0));
    assert_eq!(rows[0].priority_component, 3000.0);
}

#[test]
fn sa4_arbitration_breakdown_maps_to_bars() {
    let breakdown = ArbitrationScoreBreakdown {
        raw_pressure: 40,
        authored_weight: 1.5,
        urgency: 60.0,
        response_quality: 12.0,
        policy_component: -5.0,
        workload_penalty: 10.0,
        total: 97.0,
    };
    let bars = intent_arbitration_bars(&breakdown);
    assert_eq!(bars.len(), 7);
    assert_eq!(bars.last().map(|b| b.value), Some(97.0));
    assert_eq!(bars[0].label, "raw pressure");
}

#[test]
fn rejected_intent_reason_preserved_in_settlement_snapshot() {
    let mut world = WorldData::new(test_layout());
    let settlement_id = SettlementId::new(1);
    let plan = SettlementIntentPlan {
        settlement_id,
        planned_tick: 1,
        source_response_tick: 1,
        source_need_tick: 1,
        intents: Vec::new(),
        rejected: vec![RejectedIntentCandidate {
            response_id: ResponseId::new("grow_food"),
            need_id: NeedId::new("food"),
            candidate_score: 1.0,
            arbitration_score: 2.0,
            arbitration: ArbitrationScoreBreakdown::default(),
            reason: IntentRejectionReason::BelowScoreThreshold,
        }],
        diagnostics: Vec::new(),
    };
    world
        .settlement_intent_store_mut()
        .insert(plan);
    let mut context = CameraSettlementContext::default();
    context.focused_settlement_id = Some(settlement_id);
    let snap = build_settlement_brain_snapshot(&context, &world);
    assert_eq!(snap.rejected_intents.len(), 1);
    assert!(snap.rejected_intents[0].reason.contains("threshold"));
}

#[test]
fn spine_omits_settlement_stages_for_player_task() {
    let world = WorldData::new(test_layout());
    let task = TaskRecord::new(
        TaskId::new(1),
        TaskType::OperateWorkstation,
        TaskTarget::Building(crate::world::BuildingId::new(1)),
        TaskPriority::PlayerAssigned,
        0,
    );
    let stages = spine_stages_for_unit(
        &world,
        UnitId::new(1),
        &UnitState::Working {
            task_id: TaskId::new(1),
        },
        &CombatState::Peaceful,
        Some(&task),
    );
    let labels: Vec<_> = stages.iter().map(|s| s.label.as_str()).collect();
    assert!(labels.contains(&"Player order"));
    assert!(!labels.contains(&"Need"));
}

#[test]
fn decision_history_does_not_record_without_signal_change() {
    let world = WorldData::new(test_layout());
    let mut history = BrainDecisionHistory::default();
    record_brain_unit_history(&world, 1, &mut history);
    let count = history.len_for(UnitId::new(1));
    record_brain_unit_history(&world, 2, &mut history);
    assert_eq!(history.len_for(UnitId::new(1)), count);
}
