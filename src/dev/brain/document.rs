//! Brain UI documents — cards, inspectors, and headers from authoritative runtime data.

use crate::client::selection::WorldSelectionState;
use crate::client::CameraSettlementContext;
use crate::units::input::SelectedUnits;
use crate::world::{
    ArbitrationScoreBreakdown, CombatState, SettlementId, SettlementIntent,
    SettlementIntentPlan, StrategicTaskOrigin, TaskPriority, TaskRecord, TaskType, UnitId, UnitState,
    WorkerEvaluation, WorldData,
};

use super::history::BrainDecisionHistory;
use super::model::{
    BrainCandidateRaceRow, NO_SELECTED_UNIT_MESSAGE,
    NO_SETTLEMENT_FOCUS_MESSAGE, candidate_race_from_evaluation, intent_arbitration_bars,
};
use super::state::{BrainView, BrainWindowState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainStatusTone {
    Linked,
    Selected,
    Assigned,
    Executing,
    Score,
    Rejected,
    Neutral,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainFieldRow {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainInspectorModel {
    pub system_tag: String,
    pub title: String,
    pub rows: Vec<BrainFieldRow>,
    pub candidate_race: Vec<BrainCandidateRaceRow>,
    pub candidate_extra: u32,
    pub candidate_stale_note: Option<String>,
    pub source_records: Vec<BrainFieldRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainCardModel {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub subtitle: String,
    pub status_label: String,
    pub tone: BrainStatusTone,
    /// Provenance gap shown beside this spine card when a link is missing.
    pub link_note: Option<String>,
    pub inspector: BrainInspectorModel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainHistoryBlock {
    pub title: String,
    pub detail: String,
    pub is_latest: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitBrainDocument {
    pub empty_message: Option<String>,
    pub header_action: String,
    pub header_subtitle: String,
    pub authority_label: Option<String>,
    pub cards: Vec<BrainCardModel>,
    pub history: Vec<BrainHistoryBlock>,
    pub unit_id: Option<UnitId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettlementNeedRow {
    pub need_id: String,
    pub pressure: u8,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettlementBrainDocument {
    pub empty_message: Option<String>,
    pub settlement_name: String,
    pub header_subtitle: String,
    pub needs: Vec<SettlementNeedRow>,
    pub chain_title: String,
    pub cards: Vec<BrainCardModel>,
    pub settlement_id: Option<SettlementId>,
}

pub fn resolve_inspected_unit_id(
    brain: &BrainWindowState,
    world_selection: &WorldSelectionState,
    selected_units: &SelectedUnits,
) -> Option<UnitId> {
    if brain.view == BrainView::Unit {
        if let Some(id) = brain.unit_inspect_override {
            return Some(id);
        }
    }
    world_selection.primary_unit(selected_units)
}

pub fn build_unit_document(
    brain: &BrainWindowState,
    world_selection: &WorldSelectionState,
    selected_units: &SelectedUnits,
    world: &WorldData,
    history: &BrainDecisionHistory,
) -> UnitBrainDocument {
    let unit_id = resolve_inspected_unit_id(brain, world_selection, selected_units);
    if unit_id.is_none() {
        return UnitBrainDocument {
            empty_message: Some(NO_SELECTED_UNIT_MESSAGE.into()),
            header_action: String::new(),
            header_subtitle: String::new(),
            authority_label: None,
            cards: Vec::new(),
            history: Vec::new(),
            unit_id: None,
        };
    }
    let unit_id = unit_id.unwrap();
    let Some(unit) = world.get_unit(unit_id) else {
        return UnitBrainDocument {
            empty_message: Some(NO_SELECTED_UNIT_MESSAGE.into()),
            header_action: String::new(),
            header_subtitle: String::new(),
            authority_label: None,
            cards: Vec::new(),
            history: Vec::new(),
            unit_id: None,
        };
    };

    let task_id = world.task_store().unit_task_id(unit_id);
    let task = task_id.and_then(|id| world.task_store().get(id).cloned());
    let report = &world.worker_assignment_store().last_report;
    let evaluation = report
        .evaluations
        .iter()
        .find(|e| e.unit_id == unit_id);

    let settlement_label = unit
        .settlement_id
        .and_then(|sid| world.settlement_store().get_settlement(sid))
        .map(|r| r.display_name.clone());

    let header_action = format_unit_action_title(&unit.state, &unit.combat_state, task.as_ref());
    let mut header_subtitle = format!(
        "Unit {} - {}",
        unit_id.raw(),
        unit.definition_id.as_str()
    );
    if let Some(name) = settlement_label {
        header_subtitle.push_str(" - ");
        header_subtitle.push_str(&name);
    }
    if brain.unit_inspect_override == Some(unit_id) {
        header_subtitle.push_str(" - Inspecting from settlement");
    }

    let authority = super::model::resolve_authority_internal(&unit.combat_state, task.as_ref());
    let authority_label = authority.map(|a| a.label().to_string());

    let cards = build_unit_cards(world, unit_id, &unit.state, &unit.combat_state, task.as_ref(), evaluation, task_id);
    let history_lines = history.lines_for(unit_id);
    let history = format_history_blocks(&history_lines);

    UnitBrainDocument {
        empty_message: None,
        header_action,
        header_subtitle,
        authority_label,
        cards,
        history,
        unit_id: Some(unit_id),
    }
}

pub fn build_settlement_document(
    brain: &BrainWindowState,
    context: &CameraSettlementContext,
    world: &WorldData,
) -> SettlementBrainDocument {
    let Some(settlement_id) = context.focused_settlement_id else {
        return SettlementBrainDocument {
            empty_message: Some(NO_SETTLEMENT_FOCUS_MESSAGE.into()),
            settlement_name: String::new(),
            header_subtitle: String::new(),
            needs: Vec::new(),
            chain_title: String::new(),
            cards: Vec::new(),
            settlement_id: None,
        };
    };

    let settlement_name = world
        .settlement_store()
        .get_settlement(settlement_id)
        .map(|r| r.display_name.clone())
        .unwrap_or_else(|| format!("Settlement {}", settlement_id.raw()));

    let need_eval = world.need_evaluation_store().get(settlement_id);
    let selected_need = brain
        .selected_need_id
        .clone()
        .or_else(|| {
            need_eval
                .and_then(|e| e.snapshots.iter().max_by_key(|s| s.pressure))
                .map(|s| s.need_id.as_str().to_string())
        });

    let needs: Vec<SettlementNeedRow> = need_eval
        .map(|e| {
            e.snapshots
                .iter()
                .map(|s| SettlementNeedRow {
                    need_id: s.need_id.as_str().to_string(),
                    pressure: s.pressure,
                    selected: selected_need.as_deref() == Some(s.need_id.as_str()),
                })
                .collect()
        })
        .unwrap_or_default();

    let chain_title = selected_need
        .as_ref()
        .map(|n| format!("{} decision to work", humanize_token(n)))
        .unwrap_or_else(|| "Decision to work".into());

    let intent_plan = world.settlement_intent_store().get(settlement_id).cloned();
    let cards = build_settlement_chain(
        world,
        settlement_id,
        selected_need.as_deref(),
        intent_plan.as_ref(),
    );

    SettlementBrainDocument {
        empty_message: None,
        settlement_name,
        header_subtitle: "Settlement at camera - Latest recorded evaluations".into(),
        needs,
        chain_title,
        cards,
        settlement_id: Some(settlement_id),
    }
}

fn build_unit_cards(
    world: &WorldData,
    _unit_id: UnitId,
    state: &UnitState,
    combat: &CombatState,
    task: Option<&TaskRecord>,
    evaluation: Option<&WorkerEvaluation>,
    current_task_id: Option<crate::world::TaskId>,
) -> Vec<BrainCardModel> {
    let mut cards = Vec::new();
    let mut n = 1u32;

    if crate::world::unit_in_active_combat(combat) {
        if let Some(task) = task {
            cards.push(combat_prev_card(n, task));
            n += 1;
        }
        cards.push(combat_card(n, combat));
        n += 1;
        cards.push(action_card(n, state, combat, task, None));
        return cards;
    }

    if let Some(task) = task {
        if task.priority == TaskPriority::PlayerAssigned {
            cards.push(player_order_card(n, task));
            n += 1;
            cards.push(task_card(n, task, "Player order"));
            n += 1;
            cards.push(action_card(n, state, combat, Some(task), None));
            return cards;
        }

        if let Some(origin) = task.strategic.as_ref() {
            let need_key = need_id_from_intent_id(&origin.intent_id);
            let need_card = need_card_from_origin(world, origin, n);
            let need_linked = need_card.is_some();
            if let Some(need_card) = need_card {
                cards.push(need_card);
                n += 1;
            }
            let mut intent = intent_card(n, origin, world, settlement_id_from_origin(origin));
            if !need_linked && !need_key.is_empty() {
                intent.link_note = Some("Need link: provenance unavailable".into());
            }
            cards.push(intent);
            n += 1;
            let mut task_card_model = task_card(n, task, "Strategic task");
            if evaluation.is_none() {
                task_card_model.link_note =
                    Some("Assignment evaluation: provenance unavailable".into());
            }
            cards.push(task_card_model);
            n += 1;
            if let Some(card) = assignment_card(n, evaluation, current_task_id, task) {
                cards.push(card);
                n += 1;
            }
            cards.push(action_card(n, state, combat, Some(task), evaluation));
            return cards;
        }

        cards.push(task_card(n, task, "Task"));
        n += 1;
        cards.push(action_card(n, state, combat, Some(task), None));
        return cards;
    }

    cards.push(idle_action_card(n, state, combat));
    cards
}

fn build_settlement_chain(
    world: &WorldData,
    settlement_id: SettlementId,
    need_id: Option<&str>,
    plan: Option<&SettlementIntentPlan>,
) -> Vec<BrainCardModel> {
    let mut cards = Vec::new();
    let mut n = 1u32;
    if let Some(need_id) = need_id {
        if let Some(card) = settlement_need_card(world, settlement_id, need_id, n) {
            cards.push(card);
            n += 1;
        }
    }
    let intent = plan.and_then(|p| pick_intent_for_need(p, need_id));
    if intent.is_none() {
        return cards;
    }
    let intent = intent.unwrap();
    cards.push(settlement_intent_card(n, intent));
    n += 1;

    let propagation = world.building_intent_propagation_store().get(settlement_id);
    if let Some(report) = propagation {
        if let Some(assignment) = report
            .assignments
            .iter()
            .find(|a| a.intent_id.as_str() == intent.intent_id.as_str())
        {
            cards.push(building_card(n, assignment));
            n += 1;
        }
    }

    let task = find_strategic_task(world, intent);
    if let Some(task) = task {
        cards.push(settlement_task_card(n, &task, intent));
        n += 1;
        if let Some(worker) = task.assigned_unit_id {
            cards.push(worker_card(n, worker));
        }
    }

    cards
}

fn settlement_need_card(
    world: &WorldData,
    settlement_id: SettlementId,
    need_id: &str,
    number: u32,
) -> Option<BrainCardModel> {
    let snap = world
        .need_evaluation_store()
        .get(settlement_id)?
        .snapshots
        .iter()
        .find(|s| s.need_id.as_str() == need_id)?;
    Some(BrainCardModel {
        id: format!("need:{}", need_id),
        number,
        title: humanize_token(need_id),
        subtitle: format!("Need, pressure {}", snap.pressure),
        status_label: "Linked".into(),
        link_note: None,
        tone: BrainStatusTone::Linked,
        inspector: BrainInspectorModel {
            system_tag: "SA2 - Need snapshot".into(),
            title: humanize_token(need_id),
            rows: vec![
                row("Pressure", snap.pressure.to_string()),
                row("Current", format!("{:.1}", snap.current_value)),
                row("Desired", format!("{:.1}", snap.desired_value)),
            ],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: vec![row("need_id", need_id)],
        },
    })
}

fn pick_intent_for_need<'a>(
    plan: &'a SettlementIntentPlan,
    need_id: Option<&str>,
) -> Option<&'a SettlementIntent> {
    match need_id {
        Some(need) => plan.intents.iter().find(|i| i.source_need.as_str() == need),
        None => plan.intents.first(),
    }
}

fn find_strategic_task(world: &WorldData, intent: &SettlementIntent) -> Option<TaskRecord> {
    world
        .task_store()
        .records()
        .find(|t| {
            t.strategic
                .as_ref()
                .is_some_and(|o| o.intent_id == intent.intent_id.as_str())
        })
        .cloned()
}

fn need_card_from_origin(
    world: &WorldData,
    origin: &StrategicTaskOrigin,
    number: u32,
) -> Option<BrainCardModel> {
    let settlement_id = SettlementId::new(origin.settlement_id);
    let need_key = need_id_from_intent_id(&origin.intent_id);
    let snap = world
        .need_evaluation_store()
        .get(settlement_id)?
        .snapshots
        .iter()
        .find(|s| s.need_id.as_str() == need_key)?;
    Some(BrainCardModel {
        id: "need".into(),
        number,
        title: humanize_token(need_key),
        subtitle: format!("Need, pressure {}", snap.pressure),
        status_label: "Linked".into(),
        link_note: None,
        tone: BrainStatusTone::Linked,
        inspector: BrainInspectorModel {
            system_tag: "SA2 - Need snapshot".into(),
            title: humanize_token(need_key),
            rows: vec![
                row("Pressure", snap.pressure.to_string()),
                row("Current", format!("{:.1}", snap.current_value)),
                row("Desired", format!("{:.1}", snap.desired_value)),
                row("Evaluated tick", snap.evaluated_tick.to_string()),
            ],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: vec![
                row("need_id", snap.need_id.as_str()),
                row("evaluation_source", snap.evaluation_source.clone()),
            ],
        },
    })
}

fn intent_card(
    number: u32,
    origin: &StrategicTaskOrigin,
    world: &WorldData,
    settlement_id: SettlementId,
) -> BrainCardModel {
    let plan = world.settlement_intent_store().get(settlement_id);
    let intent = plan.and_then(|p| {
        p.intents
            .iter()
            .find(|i| i.intent_id.as_str() == origin.intent_id)
    });
    let title = origin.response_id.clone();
    let mut rows = vec![
        row("Response", origin.response_id.clone()),
        row("Intent id", origin.intent_id.clone()),
    ];
    let mut source = vec![row("template_id", origin.template_id.clone())];
    if let Some(intent) = intent {
        rows.push(row("Recorded score", format!("{:.1}", intent.priority)));
        for bar in intent_arbitration_bars(&intent.arbitration) {
            rows.push(row(bar.label, format!("{:.1}", bar.value)));
        }
        source.push(row("reasoning", intent.reasoning.clone()));
        if let Some(rejected) = plan.and_then(|p| {
            p.rejected
                .iter()
                .find(|r| r.need_id.as_str() == intent.source_need.as_str())
        }) {
            rows.push(row(
                "Other intent",
                format!("{} - {}", rejected.response_id.as_str(), rejected.reason.label()),
            ));
            rows.push(row("Recorded score", format!("{:.1}", rejected.arbitration_score)));
        }
    } else {
        rows.push(row("Plan", "Provenance unavailable"));
    }
    let (status_label, tone, link_note) = if intent.is_some() {
        ("Selected".into(), BrainStatusTone::Selected, None)
    } else {
        (
            "Unavailable".into(),
            BrainStatusTone::Unavailable,
            Some("Intent plan: provenance unavailable".into()),
        )
    };
    BrainCardModel {
        id: "intent".into(),
        number,
        title: humanize_token(&title),
        subtitle: "Selected settlement intent".into(),
        status_label,
        link_note,
        tone,
        inspector: BrainInspectorModel {
            system_tag: "SA4 - Arbitration".into(),
            title: humanize_token(&title),
            rows,
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: source,
        },
    }
}

fn task_card(number: u32, task: &TaskRecord, subtitle: &str) -> BrainCardModel {
    let building = task.target_building_id();
    BrainCardModel {
        id: "task".into(),
        number,
        title: task_title(task),
        subtitle: format!("{} - Building {}", subtitle, building.raw()),
        status_label: "Assigned".into(),
        link_note: None,
        tone: BrainStatusTone::Assigned,
        inspector: BrainInspectorModel {
            system_tag: "Task store - Strategic provenance".into(),
            title: task_title(task),
            rows: vec![
                row("Status", format!("{:?}", task.state)),
                row("Priority", format!("{:?}", task.priority)),
                row("Task type", format!("{:?}", task.task_type)),
                row("Building", building.raw().to_string()),
            ],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: strategic_source_rows(task),
        },
    }
}

fn assignment_card(
    number: u32,
    evaluation: Option<&WorkerEvaluation>,
    current_task_id: Option<crate::world::TaskId>,
    _task: &TaskRecord,
) -> Option<BrainCardModel> {
    let Some(eval) = evaluation else {
        return None;
    };
    let (race, extra, _) = candidate_race_from_evaluation(Some(eval));
    let stale = eval
        .chosen_task_id
        .zip(current_task_id)
        .filter(|(chosen, current)| chosen != current)
        .map(|_| "Latest assignment report does not match the unit's current task.".to_string());
    let score = eval.chosen_score;
    Some(BrainCardModel {
        id: "assignment".into(),
        number,
        title: format!("This task won for Unit {}", eval.unit_id.raw()),
        subtitle: "Latest assignment evaluation".into(),
        status_label: format!("Score {:.0}", score),
        link_note: None,
        tone: BrainStatusTone::Score,
        inspector: BrainInspectorModel {
            system_tag: "SA7 - Latest assignment evaluation".into(),
            title: format!("Task choices for Unit {}", eval.unit_id.raw()),
            rows: vec![
                row("Chosen task", format_task_id(eval.chosen_task_id)),
                row("Candidates", eval.candidate_count.to_string()),
                row("Report tick", "see WorkerAssignmentReport"),
            ],
            candidate_race: race,
            candidate_extra: extra,
            candidate_stale_note: stale,
            source_records: vec![
                row("unit_id", eval.unit_id.raw().to_string()),
                row("notes", eval.notes.clone()),
            ],
        },
    })
}

fn action_card(
    number: u32,
    state: &UnitState,
    combat: &CombatState,
    task: Option<&TaskRecord>,
    _eval: Option<&WorkerEvaluation>,
) -> BrainCardModel {
    let title = format_unit_action_title(state, combat, task);
    let (status_label, tone) = action_status_label_and_tone(state, combat);
    let subtitle = if tone == BrainStatusTone::Executing {
        "Current action"
    } else {
        "Current state"
    };
    BrainCardModel {
        id: "action".into(),
        number,
        title: title.clone(),
        subtitle: subtitle.into(),
        status_label,
        tone,
        link_note: None,
        inspector: BrainInspectorModel {
            system_tag: "Unit record - current execution".into(),
            title,
            rows: vec![
                row(
                    "Active combat",
                    if crate::world::unit_in_active_combat(combat) {
                        "Yes"
                    } else {
                        "No"
                    },
                ),
                row(
                    "Current task",
                    task.map(task_title).unwrap_or_else(|| "None".into()),
                ),
            ],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: Vec::new(),
        },
    }
}

fn idle_action_card(number: u32, state: &UnitState, combat: &CombatState) -> BrainCardModel {
    let title = format_unit_action_title(state, combat, None);
    BrainCardModel {
        id: "action".into(),
        number,
        title,
        subtitle: "Current state".into(),
        status_label: "Idle".into(),
        link_note: None,
        tone: BrainStatusTone::Neutral,
        inspector: BrainInspectorModel {
            system_tag: "Unit record".into(),
            title: "Idle".into(),
            rows: vec![row(
                "Active combat",
                if crate::world::unit_in_active_combat(combat) {
                    "Yes"
                } else {
                    "No"
                },
            )],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: Vec::new(),
        },
    }
}

fn action_status_label_and_tone(
    state: &UnitState,
    combat: &CombatState,
) -> (String, BrainStatusTone) {
    if crate::world::unit_in_active_combat(combat) {
        return ("Combat".into(), BrainStatusTone::Executing);
    }
    match state {
        UnitState::Idle => ("Idle".into(), BrainStatusTone::Neutral),
        UnitState::Dead => ("Dead".into(), BrainStatusTone::Unavailable),
        UnitState::Moving { .. } | UnitState::Working { .. } => {
            ("Executing".into(), BrainStatusTone::Executing)
        }
    }
}

fn player_order_card(number: u32, task: &TaskRecord) -> BrainCardModel {
    BrainCardModel {
        id: "player_order".into(),
        number,
        title: "Player order".into(),
        subtitle: task_title(task),
        status_label: "Player".into(),
        link_note: None,
        tone: BrainStatusTone::Selected,
        inspector: BrainInspectorModel {
            system_tag: "Player assignment".into(),
            title: "Player order".into(),
            rows: vec![row("Task", task_title(task))],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: strategic_source_rows(task),
        },
    }
}

fn combat_prev_card(number: u32, task: &TaskRecord) -> BrainCardModel {
    BrainCardModel {
        id: "combat_prev".into(),
        number,
        title: "Previous work".into(),
        subtitle: task_title(task),
        status_label: "Linked".into(),
        link_note: None,
        tone: BrainStatusTone::Linked,
        inspector: BrainInspectorModel {
            system_tag: "Task store".into(),
            title: "Previous work".into(),
            rows: vec![row("Task", task_title(task))],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: strategic_source_rows(task),
        },
    }
}

fn combat_card(number: u32, combat: &CombatState) -> BrainCardModel {
    BrainCardModel {
        id: "combat".into(),
        number,
        title: "Combat override".into(),
        subtitle: format!("{:?}", combat),
        status_label: "Combat".into(),
        link_note: None,
        tone: BrainStatusTone::Executing,
        inspector: BrainInspectorModel {
            system_tag: "Combat state".into(),
            title: "Combat override".into(),
            rows: vec![row("State", format!("{:?}", combat))],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: Vec::new(),
        },
    }
}

fn settlement_intent_card(number: u32, intent: &SettlementIntent) -> BrainCardModel {
    let title = intent.chosen_response.as_str();
    BrainCardModel {
        id: format!("intent:{}", intent.intent_id.as_str()),
        number,
        title: humanize_token(title),
        subtitle: "Selected intent".into(),
        status_label: "Selected".into(),
        link_note: None,
        tone: BrainStatusTone::Selected,
        inspector: BrainInspectorModel {
            system_tag: "SA4 - Arbitration".into(),
            title: humanize_token(title),
            rows: arbitration_rows(&intent.arbitration, intent),
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: vec![row("intent_id", intent.intent_id.as_str())],
        },
    }
}

fn building_card(
    number: u32,
    assignment: &crate::world::BuildingPolicyAssignment,
) -> BrainCardModel {
    BrainCardModel {
        id: format!("building:{}", assignment.building_id.raw()),
        number,
        title: format!("Building {}", assignment.building_id.raw()),
        subtitle: "Building intent propagation".into(),
        status_label: "Linked".into(),
        link_note: None,
        tone: BrainStatusTone::Linked,
        inspector: BrainInspectorModel {
            system_tag: "SA5 - Propagation".into(),
            title: format!("Building {}", assignment.building_id.raw()),
            rows: vec![
                row("Originating intent", assignment.intent_id.as_str()),
                row("Response", assignment.response_id.as_str()),
                row("Enabled", assignment.enabled.to_string()),
                row("Reason", assignment.reason.clone()),
            ],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: vec![row("need_id", assignment.need_id.as_str())],
        },
    }
}

fn settlement_task_card(number: u32, task: &TaskRecord, intent: &SettlementIntent) -> BrainCardModel {
    BrainCardModel {
        id: format!("task:{}", task.id.raw()),
        number,
        title: task_title(task),
        subtitle: "Strategic task".into(),
        status_label: "Assigned".into(),
        link_note: None,
        tone: BrainStatusTone::Assigned,
        inspector: BrainInspectorModel {
            system_tag: "SA6 - Strategic task".into(),
            title: task_title(task),
            rows: vec![
                row("Originating intent", intent.intent_id.as_str()),
                row(
                    "Worker",
                    task.assigned_unit_id
                        .map(|u| u.raw().to_string())
                        .unwrap_or_else(|| "Unassigned".into()),
                ),
            ],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: strategic_source_rows(task),
        },
    }
}

fn worker_card(number: u32, unit_id: UnitId) -> BrainCardModel {
    BrainCardModel {
        id: format!("worker:{}", unit_id.raw()),
        number,
        title: format!("Unit {}", unit_id.raw()),
        subtitle: "Assigned worker".into(),
        status_label: "Inspect".into(),
        link_note: None,
        tone: BrainStatusTone::Linked,
        inspector: BrainInspectorModel {
            system_tag: "Worker".into(),
            title: format!("Unit {}", unit_id.raw()),
            rows: vec![row("Action", "Open Unit Brain")],
            candidate_race: Vec::new(),
            candidate_extra: 0,
            candidate_stale_note: None,
            source_records: vec![row("unit_id", unit_id.raw().to_string())],
        },
    }
}

fn arbitration_rows(
    breakdown: &ArbitrationScoreBreakdown,
    intent: &SettlementIntent,
) -> Vec<BrainFieldRow> {
    let mut rows = vec![
        row("Outcome", "Selected"),
        row("Originating need", intent.source_need.as_str()),
        row("Recorded score", format!("{:.1}", breakdown.total)),
    ];
    for bar in intent_arbitration_bars(breakdown) {
        rows.push(row(bar.label, format!("{:.1}", bar.value)));
    }
    rows
}

fn format_history_blocks(lines: &[String]) -> Vec<BrainHistoryBlock> {
    if lines.is_empty() {
        return Vec::new();
    }
    let last = lines.len() - 1;
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let (title, detail) = parse_history_line(line);
            BrainHistoryBlock {
                title,
                detail,
                is_latest: i == last,
            }
        })
        .collect()
}

fn parse_history_line(line: &str) -> (String, String) {
    if let Some(rest) = line.strip_prefix("tick ") {
        if let Some((_, after)) = rest.split_once(": ") {
            return ("Transition".into(), after.to_string());
        }
    }
    ("Transition".into(), line.to_string())
}

fn row(label: impl Into<String>, value: impl Into<String>) -> BrainFieldRow {
    BrainFieldRow {
        label: label.into(),
        value: value.into(),
    }
}

fn humanize_token(s: &str) -> String {
    s.replace('_', " ")
}

fn need_id_from_intent_id(intent_id: &str) -> &str {
    intent_id.split(':').nth(2).unwrap_or("")
}

fn settlement_id_from_origin(origin: &StrategicTaskOrigin) -> SettlementId {
    SettlementId::new(origin.settlement_id)
}

fn task_title(task: &TaskRecord) -> String {
    match task.task_type {
        TaskType::Haul => "Haul".into(),
        TaskType::OperateWorkstation => "Operate workstation".into(),
        TaskType::ConstructBuilding => "Construct building".into(),
        TaskType::StrategicConstruct => "Strategic construct".into(),
        TaskType::RepairBuilding => "Repair building".into(),
        TaskType::ClearRubble => "Clear rubble".into(),
        TaskType::RecruitWorker => "Recruit worker".into(),
        TaskType::ExpandStorage => "Expand storage".into(),
    }
}

fn format_task_id(id: Option<crate::world::TaskId>) -> String {
    id.map(|t| t.raw().to_string()).unwrap_or_else(|| "None".into())
}

fn strategic_source_rows(task: &TaskRecord) -> Vec<BrainFieldRow> {
    task.strategic
        .as_ref()
        .map(|o| {
            vec![
                row("settlement_id", o.settlement_id.to_string()),
                row("intent_id", o.intent_id.clone()),
                row("response_id", o.response_id.clone()),
                row("template_id", o.template_id.clone()),
            ]
        })
        .unwrap_or_default()
}

fn format_unit_action_title(
    state: &UnitState,
    combat: &CombatState,
    task: Option<&TaskRecord>,
) -> String {
    if crate::world::unit_in_active_combat(combat) {
        return "Combat".into();
    }
    match state {
        UnitState::Idle => "Idle".into(),
        UnitState::Moving { .. } => "Moving".into(),
        UnitState::Working { .. } => task
            .map(task_title)
            .unwrap_or_else(|| "Working".into()),
        UnitState::Dead => "Dead".into(),
    }
}
