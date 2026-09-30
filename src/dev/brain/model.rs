//! Pure Brain view models from authoritative runtime stores (no invented explanations).

use crate::client::selection::WorldSelectionState;
use crate::client::CameraSettlementContext;
use crate::units::input::SelectedUnits;
use crate::world::{
    ArbitrationScoreBreakdown, CombatState, NeedSnapshot,
    RejectedIntentCandidate, SettlementId, SettlementIntent, SettlementIntentPlan,
    SettlementNeedEvaluation, StrategicTaskOrigin, TaskPriority, TaskRecord, UnitId, UnitState,
    WorkerCandidateDiagnostic, WorkerEvaluation, WorldData,
};

pub const NO_SELECTED_UNIT_MESSAGE: &str = "No unit selected";
pub const NO_SETTLEMENT_FOCUS_MESSAGE: &str = "No focused settlement";
pub const MAX_CANDIDATE_RACE_ALTERNATES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainAuthoritySource {
    Player,
    Settlement,
    Combat,
    SelfDriven,
}

impl BrainAuthoritySource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Player => "Player",
            Self::Settlement => "Settlement",
            Self::Combat => "Combat",
            Self::SelfDriven => "Self",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainSpineStage {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainCandidateRaceRow {
    pub label: String,
    pub total_score: f32,
    pub priority_component: f32,
    pub distance_component: f32,
    pub chosen: bool,
    pub eligible: bool,
    pub block_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainScoreBar {
    pub label: String,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainUnitSnapshot {
    pub unit_id: Option<UnitId>,
    pub unit_label: String,
    pub action_summary: String,
    pub task_summary: Option<String>,
    pub authority: Option<BrainAuthoritySource>,
    pub spine: Vec<BrainSpineStage>,
    pub candidate_race: Vec<BrainCandidateRaceRow>,
    pub extra_candidates: u32,
    pub blocked_alternatives: Vec<String>,
    pub history: Vec<String>,
    pub empty_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainNeedPressureRow {
    pub need_id: String,
    pub pressure: u8,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainIntentRow {
    pub intent_id: String,
    pub response_id: String,
    pub need_id: String,
    pub priority: f32,
    pub arbitration_bars: Vec<BrainScoreBar>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainRejectedIntentRow {
    pub response_id: String,
    pub need_id: String,
    pub reason: String,
    pub arbitration_total: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainDownstreamLink {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainSettlementSnapshot {
    pub settlement_id: Option<SettlementId>,
    pub settlement_label: String,
    pub need_pressures: Vec<BrainNeedPressureRow>,
    pub selected_intents: Vec<BrainIntentRow>,
    pub rejected_intents: Vec<BrainRejectedIntentRow>,
    pub downstream: Vec<BrainDownstreamLink>,
    pub empty_message: Option<String>,
}

pub fn build_unit_brain_snapshot(
    world_selection: &WorldSelectionState,
    selected_units: &SelectedUnits,
    world: &WorldData,
    evaluation: Option<&WorkerEvaluation>,
    history_lines: &[String],
) -> BrainUnitSnapshot {
    let unit_id = world_selection.primary_unit(selected_units);
    if unit_id.is_none() {
        return empty_unit_snapshot(NO_SELECTED_UNIT_MESSAGE);
    }
    let unit_id = unit_id.unwrap();
    let Some(unit) = world.get_unit(unit_id) else {
        return empty_unit_snapshot(NO_SELECTED_UNIT_MESSAGE);
    };

    let task_id = world.task_store().unit_task_id(unit_id);
    let task = task_id.and_then(|id| world.task_store().get(id).cloned());
    let authority = resolve_authority_internal(&unit.combat_state, task.as_ref());
    let action_summary = format_unit_action(&unit.state, &unit.combat_state);
    let task_summary = task.as_ref().map(format_task_line);
    let spine = spine_stages_for_unit(world, unit_id, &unit.state, &unit.combat_state, task.as_ref());
    let (candidate_race, extra_candidates, blocked_alternatives) =
        candidate_race_from_evaluation(evaluation.filter(|e| e.unit_id == unit_id));

    BrainUnitSnapshot {
        unit_id: Some(unit_id),
        unit_label: format!("Unit {} ({})", unit_id.raw(), unit.definition_id.as_str()),
        action_summary,
        task_summary,
        authority,
        spine,
        candidate_race,
        extra_candidates,
        blocked_alternatives,
        history: history_lines.to_vec(),
        empty_message: None,
    }
}

fn empty_unit_snapshot(message: &str) -> BrainUnitSnapshot {
    BrainUnitSnapshot {
        unit_id: None,
        unit_label: String::new(),
        action_summary: String::new(),
        task_summary: None,
        authority: None,
        spine: Vec::new(),
        candidate_race: Vec::new(),
        extra_candidates: 0,
        blocked_alternatives: Vec::new(),
        history: Vec::new(),
        empty_message: Some(message.into()),
    }
}

pub fn build_settlement_brain_snapshot(
    context: &CameraSettlementContext,
    world: &WorldData,
) -> BrainSettlementSnapshot {
    let Some(settlement_id) = context.focused_settlement_id else {
        return BrainSettlementSnapshot {
            settlement_id: None,
            settlement_label: String::new(),
            need_pressures: Vec::new(),
            selected_intents: Vec::new(),
            rejected_intents: Vec::new(),
            downstream: Vec::new(),
            empty_message: Some(NO_SETTLEMENT_FOCUS_MESSAGE.into()),
        };
    };

    let settlement_label = world
        .settlement_store()
        .get_settlement(settlement_id)
        .map(|r| r.display_name.clone())
        .unwrap_or_else(|| format!("Settlement {}", settlement_id.raw()));

    let need_pressures = world
        .need_evaluation_store()
        .get(settlement_id)
        .map(need_pressure_rows)
        .unwrap_or_default();

    let intent_plan = world.settlement_intent_store().get(settlement_id).cloned();
    let selected_intents = intent_plan
        .as_ref()
        .map(selected_intent_rows)
        .unwrap_or_default();
    let rejected_intents = intent_plan
        .as_ref()
        .map(rejected_intent_rows)
        .unwrap_or_default();

    let downstream = downstream_links(world, settlement_id, intent_plan.as_ref());

    BrainSettlementSnapshot {
        settlement_id: Some(settlement_id),
        settlement_label,
        need_pressures,
        selected_intents,
        rejected_intents,
        downstream,
        empty_message: None,
    }
}

pub(crate) fn resolve_authority_internal(
    combat_state: &CombatState,
    task: Option<&TaskRecord>,
) -> Option<BrainAuthoritySource> {
    if crate::world::unit_in_active_combat(combat_state) {
        return Some(BrainAuthoritySource::Combat);
    }
    if let Some(task) = task {
        if task.priority == TaskPriority::PlayerAssigned {
            return Some(BrainAuthoritySource::Player);
        }
        if task.strategic.is_some() {
            return Some(BrainAuthoritySource::Settlement);
        }
    }
    None
}

fn format_unit_action(state: &UnitState, combat: &CombatState) -> String {
    if crate::world::unit_in_active_combat(combat) {
        return format!("Combat: {:?}", combat);
    }
    match state {
        UnitState::Idle => "Idle".into(),
        UnitState::Moving { .. } => "Moving".into(),
        UnitState::Working { task_id } => format!("Working (task {})", task_id.raw()),
        UnitState::Dead => "Dead".into(),
    }
}

fn format_task_line(task: &TaskRecord) -> String {
    let origin = task
        .strategic
        .as_ref()
        .map(|o| format!(" intent={}", o.intent_id))
        .unwrap_or_default();
    format!(
        "Task {} {:?} {:?}{}",
        task.id.raw(),
        task.task_type,
        task.priority,
        origin
    )
}

pub fn spine_stages_for_unit(
    world: &WorldData,
    unit_id: UnitId,
    state: &UnitState,
    combat_state: &CombatState,
    task: Option<&TaskRecord>,
) -> Vec<BrainSpineStage> {
    let mut stages = Vec::new();

    if crate::world::unit_in_active_combat(combat_state) {
        if let Some(task) = task {
            stages.push(BrainSpineStage {
                label: "Previous work".into(),
                detail: format_task_line(task),
            });
        }
        stages.push(BrainSpineStage {
            label: "Combat override".into(),
            detail: format!("{:?}", combat_state),
        });
        stages.push(BrainSpineStage {
            label: "Action".into(),
            detail: format_unit_action(state, combat_state),
        });
        return stages;
    }

    if let Some(task) = task {
        if task.priority == TaskPriority::PlayerAssigned {
            stages.push(BrainSpineStage {
                label: "Player order".into(),
                detail: format_task_line(task),
            });
            stages.push(BrainSpineStage {
                label: "Task".into(),
                detail: format!("{}", task.id.raw()),
            });
            stages.push(BrainSpineStage {
                label: "Action".into(),
                detail: format_unit_action(state, combat_state),
            });
            return stages;
        }

        if let Some(origin) = task.strategic.as_ref() {
            append_settlement_spine(world, &mut stages, origin, task);
            if let Some(eval) = world
                .worker_assignment_store()
                .last_report
                .evaluations
                .iter()
                .find(|e| e.unit_id == unit_id)
            {
                if eval.chosen_task_id == Some(task.id) {
                    stages.push(BrainSpineStage {
                        label: "Worker choice (SA7)".into(),
                        detail: format!(
                            "score={:.1} candidates={}",
                            eval.chosen_score,
                            eval.candidate_count
                        ),
                    });
                }
            }
            stages.push(BrainSpineStage {
                label: "Action".into(),
                detail: format_unit_action(state, combat_state),
            });
            return stages;
        }

        stages.push(BrainSpineStage {
            label: "Task".into(),
            detail: format_task_line(task),
        });
        stages.push(BrainSpineStage {
            label: "Action".into(),
            detail: format_unit_action(state, combat_state),
        });
        return stages;
    }

    stages.push(BrainSpineStage {
        label: "Action".into(),
        detail: format_unit_action(state, combat_state),
    });
    stages
}

fn append_settlement_spine(
    world: &WorldData,
    stages: &mut Vec<BrainSpineStage>,
    origin: &StrategicTaskOrigin,
    task: &TaskRecord,
) {
    let settlement_id = SettlementId::new(origin.settlement_id);
    if let Some(eval) = world.need_evaluation_store().get(settlement_id) {
        let need_key = need_id_from_task_origin(origin);
        if let Some(need) = eval.snapshots.iter().find(|s| s.need_id.as_str() == need_key) {
            stages.push(BrainSpineStage {
                label: "Need".into(),
                detail: format!("{} pressure={}", need.need_id.as_str(), need.pressure),
            });
        }
    }
    stages.push(BrainSpineStage {
        label: "Response".into(),
        detail: origin.response_id.clone(),
    });
    stages.push(BrainSpineStage {
        label: "Intent".into(),
        detail: origin.intent_id.clone(),
    });
    stages.push(BrainSpineStage {
        label: "Task (SA6)".into(),
        detail: format_task_line(task),
    });
}

fn need_id_from_task_origin(origin: &StrategicTaskOrigin) -> &str {
    origin.intent_id.split(':').nth(2).unwrap_or("")
}

pub fn candidate_race_rows(
    evaluation: Option<&WorkerEvaluation>,
) -> (Vec<BrainCandidateRaceRow>, u32, Vec<String>) {
    candidate_race_from_evaluation(evaluation)
}

pub(crate) fn candidate_race_from_evaluation(
    evaluation: Option<&WorkerEvaluation>,
) -> (Vec<BrainCandidateRaceRow>, u32, Vec<String>) {
    let Some(eval) = evaluation else {
        return (Vec::new(), 0, Vec::new());
    };
    let eligible: Vec<_> = eval
        .ranked_candidates
        .iter()
        .filter(|c| c.eligible)
        .collect();
    let chosen_id = eval.chosen_task_id;
    let mut rows: Vec<BrainCandidateRaceRow> = eligible
        .iter()
        .map(|c| diagnostic_to_race_row(c, chosen_id))
        .collect();
    rows.sort_by(|a, b| {
        b.total_score
            .partial_cmp(&a.total_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let display_count = 1 + MAX_CANDIDATE_RACE_ALTERNATES;
    let extra = rows.len().saturating_sub(display_count) as u32;
    rows.truncate(display_count);

    let blocked: Vec<String> = eval
        .ranked_candidates
        .iter()
        .filter(|c| !c.eligible)
        .take(3)
        .map(|c| {
            let reason = c
                .block_reason
                .clone()
                .unwrap_or_else(|| "ineligible".into());
            format!(
                "{:?} task={:?}: {}",
                c.task_type,
                c.task_id.map(|id| id.raw()),
                reason
            )
        })
        .collect();

    (rows, extra, blocked)
}

fn diagnostic_to_race_row(
    c: &WorkerCandidateDiagnostic,
    chosen_id: Option<crate::world::TaskId>,
) -> BrainCandidateRaceRow {
    let chosen = chosen_id.is_some() && c.task_id == chosen_id;
    BrainCandidateRaceRow {
        label: format!("{:?} ({:?})", c.task_type, c.priority),
        total_score: c.total_score,
        priority_component: c.priority_component,
        distance_component: c.distance_component,
        chosen,
        eligible: c.eligible,
        block_reason: c.block_reason.clone(),
    }
}

pub fn intent_arbitration_bars(breakdown: &ArbitrationScoreBreakdown) -> Vec<BrainScoreBar> {
    vec![
        BrainScoreBar {
            label: "raw pressure".into(),
            value: f32::from(breakdown.raw_pressure),
        },
        BrainScoreBar {
            label: "authored weight".into(),
            value: breakdown.authored_weight,
        },
        BrainScoreBar {
            label: "urgency".into(),
            value: breakdown.urgency,
        },
        BrainScoreBar {
            label: "response quality".into(),
            value: breakdown.response_quality,
        },
        BrainScoreBar {
            label: "policy".into(),
            value: breakdown.policy_component,
        },
        BrainScoreBar {
            label: "workload penalty".into(),
            value: -breakdown.workload_penalty,
        },
        BrainScoreBar {
            label: "total".into(),
            value: breakdown.total,
        },
    ]
}

fn need_pressure_rows(eval: &SettlementNeedEvaluation) -> Vec<BrainNeedPressureRow> {
    let mut rows: Vec<_> = eval
        .snapshots
        .iter()
        .map(|snap| BrainNeedPressureRow {
            need_id: snap.need_id.as_str().to_string(),
            pressure: snap.pressure,
            detail: Some(format_need_detail(snap)),
        })
        .collect();
    rows.sort_by(|a, b| b.pressure.cmp(&a.pressure));
    rows
}

fn format_need_detail(snap: &NeedSnapshot) -> String {
    let block = snap
        .blocking_reason
        .as_ref()
        .map(|r| format!(" block={}", r.label()))
        .unwrap_or_default();
    format!(
        "cur={:.1} want={:.1} deficit={:.1} surplus={:.1} src={} tick={}{}",
        snap.current_value,
        snap.desired_value,
        snap.deficit,
        snap.surplus,
        snap.evaluation_source,
        snap.evaluated_tick,
        block
    )
}

fn selected_intent_rows(plan: &SettlementIntentPlan) -> Vec<BrainIntentRow> {
    plan.intents.iter().take(6).map(intent_row).collect()
}

fn intent_row(intent: &SettlementIntent) -> BrainIntentRow {
    BrainIntentRow {
        intent_id: intent.intent_id.as_str().to_string(),
        response_id: intent.chosen_response.as_str().to_string(),
        need_id: intent.source_need.as_str().to_string(),
        priority: intent.priority,
        arbitration_bars: intent_arbitration_bars(&intent.arbitration),
    }
}

fn rejected_intent_rows(plan: &SettlementIntentPlan) -> Vec<BrainRejectedIntentRow> {
    let mut rows: Vec<_> = plan.rejected.iter().map(rejected_row).collect();
    rows.sort_by(|a, b| {
        b.arbitration_total
            .partial_cmp(&a.arbitration_total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(5);
    rows
}

fn rejected_row(candidate: &RejectedIntentCandidate) -> BrainRejectedIntentRow {
    BrainRejectedIntentRow {
        response_id: candidate.response_id.as_str().to_string(),
        need_id: candidate.need_id.as_str().to_string(),
        reason: candidate.reason.label(),
        arbitration_total: candidate.arbitration_score,
    }
}

fn downstream_links(
    world: &WorldData,
    settlement_id: SettlementId,
    plan: Option<&SettlementIntentPlan>,
) -> Vec<BrainDownstreamLink> {
    let mut links = Vec::new();
    if let Some(report) = world.building_intent_propagation_store().get(settlement_id) {
        links.push(BrainDownstreamLink {
            label: "SA5 building policy".into(),
            detail: format!(
                "tick={} assignments={}",
                report.propagated_tick,
                report.assignments.len()
            ),
        });
    }
    let open_tasks = world
        .task_store()
        .records()
        .filter(|t| {
            t.strategic
                .as_ref()
                .is_some_and(|o| o.settlement_id == settlement_id.raw())
        })
        .count();
    if open_tasks > 0 {
        links.push(BrainDownstreamLink {
            label: "SA6 strategic tasks".into(),
            detail: format!("{} task(s) linked to settlement", open_tasks),
        });
    }
    let report = &world.worker_assignment_store().last_report;
    if report.generated_tick > 0 {
        links.push(BrainDownstreamLink {
            label: "SA7 worker marketplace".into(),
            detail: format!(
                "tick={} idle={} open={}",
                report.generated_tick,
                report.idle_workers,
                report.open_listings
            ),
        });
    }
    if let Some(plan) = plan {
        if !plan.intents.is_empty() {
            links.push(BrainDownstreamLink {
                label: "Note".into(),
                detail: "Settlement chooses intents; SA7 assigns workers to tasks.".into(),
            });
        }
    }
    links
}
