//! Emit / merge / cancel strategic tasks from SettlementIntent (SA6).

use crate::world::settlement::arbiter::{SettlementIntent, SettlementIntentPlan};
use crate::world::settlement::response::ResponseType;
use crate::world::task::{
    ensure_building_task, StrategicTaskOrigin, TaskId, TaskPriority, TaskRecord, TaskState,
    TaskTarget, TaskType,
};
use crate::world::{BuildingId, WorldData};

use super::catalog::StrategicTaskTemplateCatalog;
use super::report::{StrategicTaskEmission, StrategicTaskGenerationReport};
use super::template::StrategicTaskTemplate;

pub struct StrategicTaskGenContext<'a> {
    pub world: &'a mut WorldData,
    pub catalog: &'a StrategicTaskTemplateCatalog,
    pub intent_plan: &'a SettlementIntentPlan,
    pub simulation_tick: u64,
}

/// Generate strategic tasks for one settlement from its intent plan.
///
/// Does not assign workers. Does not emit OperateWorkstation / Haul (production/logistics stay
/// owned by their runtimes).
pub fn generate_strategic_tasks_for_settlement(
    ctx: &mut StrategicTaskGenContext<'_>,
) -> StrategicTaskGenerationReport {
    let mut report = StrategicTaskGenerationReport {
        settlement_id: ctx.intent_plan.settlement_id,
        generated_tick: ctx.simulation_tick,
        source_intent_tick: ctx.intent_plan.planned_tick,
        emissions: Vec::new(),
        cancelled_task_ids: Vec::new(),
        diagnostics: Vec::new(),
    };

    let mut desired_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let anchor = settlement_anchor_building(ctx.world, ctx.intent_plan.settlement_id);

    for intent in &ctx.intent_plan.intents {
        // Production / research intents are policy-driven (SA5) — not task-gen.
        if matches!(
            intent.response_type,
            ResponseType::IncreaseProduction
                | ResponseType::DecreaseProduction
                | ResponseType::Research
                | ResponseType::Trade
        ) {
            continue;
        }

        let templates = ctx
            .catalog
            .templates_for_response(&intent.chosen_response, intent.response_type);
        if templates.is_empty() {
            report.diagnostics.push(format!(
                "no strategic task template for response `{}`",
                intent.chosen_response.as_str()
            ));
            continue;
        }

        for template in templates {
            match emit_for_intent(ctx, intent, template, anchor, &mut report) {
                Ok(keys) => desired_keys.extend(keys),
                Err(diag) => report.diagnostics.push(diag),
            }
        }
    }

    cancel_stale_strategic_tasks(ctx, &desired_keys, &mut report);
    report.diagnostics.push(format!(
        "emissions={} cancelled={} desired_keys={}",
        report.emissions.len(),
        report.cancelled_task_ids.len(),
        desired_keys.len()
    ));
    report
}

fn emit_for_intent(
    ctx: &mut StrategicTaskGenContext<'_>,
    intent: &SettlementIntent,
    template: &StrategicTaskTemplate,
    anchor: Option<BuildingId>,
    report: &mut StrategicTaskGenerationReport,
) -> Result<Vec<String>, String> {
    let mut priority = intent_to_task_priority(intent.priority);
    // SA8: optional one-tier bump from authored emergency task modifiers (not a second pressure apply).
    let emergency_catalog = crate::world::settlement::emergency::EmergencyCatalog::default();
    if let Some(state) = ctx.world.settlement_state_store().get(ctx.intent_plan.settlement_id) {
        priority = crate::world::settlement::emergency::emergency_bump_task_priority(
            state,
            &emergency_catalog,
            intent.chosen_response.as_str(),
            priority,
        );
    }
    let origin = StrategicTaskOrigin {
        settlement_id: ctx.intent_plan.settlement_id.raw(),
        intent_id: intent.intent_id.as_str().to_string(),
        response_id: intent.chosen_response.as_str().to_string(),
        template_id: template.id.as_str().to_string(),
    };

    let mut keys = Vec::new();

    if template.prefer_construction_sites {
        let sites = constructible_settlement_buildings(ctx.world, ctx.intent_plan.settlement_id);
        if !sites.is_empty() {
            for building_id in sites {
                let key = merge_key(&origin, Some(building_id), TaskType::ConstructBuilding);
                let task_id = upsert_strategic_task(
                    ctx.world,
                    building_id,
                    TaskType::ConstructBuilding,
                    priority,
                    origin.clone(),
                    ctx.simulation_tick,
                    &key,
                )?;
                keys.push(key.clone());
                report.emissions.push(StrategicTaskEmission {
                    task_id,
                    intent_id: intent.intent_id.as_str().to_string(),
                    response_id: intent.chosen_response.as_str().to_string(),
                    template_id: template.id.as_str().to_string(),
                    task_type: TaskType::ConstructBuilding,
                    building_id,
                    priority,
                    merged: true,
                    reason: format!(
                        "construction site #{} from response `{}`",
                        building_id.raw(),
                        intent.chosen_response.as_str()
                    ),
                });
            }
            return Ok(keys);
        }
    }

    let Some(building_id) = anchor else {
        return Err(format!(
            "response `{}`: no settlement anchor building for strategic task",
            intent.chosen_response.as_str()
        ));
    };

    let key = merge_key(&origin, Some(building_id), template.task_type);
    let task_id = upsert_strategic_task(
        ctx.world,
        building_id,
        template.task_type,
        priority,
        origin,
        ctx.simulation_tick,
        &key,
    )?;
    keys.push(key);
    report.emissions.push(StrategicTaskEmission {
        task_id,
        intent_id: intent.intent_id.as_str().to_string(),
        response_id: intent.chosen_response.as_str().to_string(),
        template_id: template.id.as_str().to_string(),
        task_type: template.task_type,
        building_id,
        priority,
        merged: true,
        reason: format!(
            "template `{}` → {:?} on anchor #{}",
            template.id.as_str(),
            template.task_type,
            building_id.raw()
        ),
    });
    Ok(keys)
}

fn upsert_strategic_task(
    world: &mut WorldData,
    building_id: BuildingId,
    task_type: TaskType,
    priority: TaskPriority,
    origin: StrategicTaskOrigin,
    simulation_tick: u64,
    merge_key: &str,
) -> Result<TaskId, String> {
    // Merge: same strategic key on an active task.
    for task_id in world.task_store().sorted_task_ids() {
        let Some(task) = world.task_store().get(task_id) else {
            continue;
        };
        if !matches!(
            task.state,
            TaskState::Available | TaskState::Assigned | TaskState::InProgress | TaskState::BlockedWaiting
        ) {
            continue;
        }
        if let Some(existing) = &task.strategic {
            let existing_key = merge_key_from_origin(existing, task.target_building_id(), task.task_type);
            if existing_key == merge_key {
                if let Some(task) = world.task_store_mut().get_mut(task_id) {
                    // Refresh priority from current intent; keep assignment.
                    if task.priority != TaskPriority::PlayerAssigned {
                        task.priority = priority;
                    }
                    task.strategic = Some(origin);
                }
                return Ok(task_id);
            }
        }
    }

    if task_type == TaskType::ConstructBuilding {
        let task_id = ensure_building_task(world, building_id, task_type, priority, simulation_tick)
            .map_err(|e| format!("{e:?}"))?;
        if let Some(task) = world.task_store_mut().get_mut(task_id) {
            if task.priority != TaskPriority::PlayerAssigned {
                task.priority = priority;
            }
            task.strategic = Some(origin);
        }
        return Ok(task_id);
    }

    let task_id = world.task_store_mut().allocate_task_id();
    let record = TaskRecord::new(
        task_id,
        task_type,
        TaskTarget::Building(building_id),
        priority,
        simulation_tick,
    )
    .with_strategic(origin);
    world
        .task_store_mut()
        .insert_task(record)
        .map_err(|e| format!("{e:?}"))?;
    Ok(task_id)
}

fn cancel_stale_strategic_tasks(
    ctx: &mut StrategicTaskGenContext<'_>,
    desired_keys: &std::collections::BTreeSet<String>,
    report: &mut StrategicTaskGenerationReport,
) {
    let settlement_raw = ctx.intent_plan.settlement_id.raw();
    let task_ids = ctx.world.task_store().sorted_task_ids();
    for task_id in task_ids {
        let Some(task) = ctx.world.task_store().get(task_id).cloned() else {
            continue;
        };
        let Some(origin) = &task.strategic else {
            continue;
        };
        if origin.settlement_id != settlement_raw {
            continue;
        }
        // Only cancel Available strategic tasks (Assigned/InProgress continue until done).
        if task.state != TaskState::Available {
            continue;
        }
        let key = merge_key_from_origin(origin, task.target_building_id(), task.task_type);
        if desired_keys.contains(&key) {
            continue;
        }
        ctx.world.task_store_mut().remove_task(task_id);
        report.cancelled_task_ids.push(task_id);
        report.diagnostics.push(format!(
            "cancelled stale strategic task #{} ({})",
            task_id.raw(),
            origin.template_id
        ));
    }
}

pub fn intent_to_task_priority(intent_priority: f32) -> TaskPriority {
    if !intent_priority.is_finite() {
        return TaskPriority::Normal;
    }
    if intent_priority >= 120.0 {
        TaskPriority::High
    } else if intent_priority >= 40.0 {
        TaskPriority::Normal
    } else {
        TaskPriority::Low
    }
}

fn merge_key(origin: &StrategicTaskOrigin, building_id: Option<BuildingId>, task_type: TaskType) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        origin.settlement_id,
        origin.template_id,
        origin.response_id,
        building_id.map(|b| b.raw()).unwrap_or(0),
        task_type.label()
    )
}

fn merge_key_from_origin(
    origin: &StrategicTaskOrigin,
    building_id: BuildingId,
    task_type: TaskType,
) -> String {
    merge_key(origin, Some(building_id), task_type)
}

fn settlement_anchor_building(
    world: &WorldData,
    settlement_id: crate::world::settlement::SettlementId,
) -> Option<BuildingId> {
    world
        .settlement_store()
        .get_settlement(settlement_id)
        .map(|record| record.anchor_building_id)
}

fn constructible_settlement_buildings(
    world: &WorldData,
    settlement_id: crate::world::settlement::SettlementId,
) -> Vec<BuildingId> {
    let mut ids = Vec::new();
    for building_id in world
        .settlement_store()
        .buildings_for_settlement(settlement_id)
    {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if crate::world::task::building_is_constructible(record) {
            ids.push(building_id);
        }
    }
    ids.sort_by_key(|id| id.raw());
    ids
}
