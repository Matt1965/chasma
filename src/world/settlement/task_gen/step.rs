//! Dirty/cadence strategic task generation step (SA6).

use crate::world::settlement::SettlementId;
use crate::world::WorldData;

use super::catalog::StrategicTaskTemplateCatalog;
use super::emit::{generate_strategic_tasks_for_settlement, StrategicTaskGenContext};

pub const STRATEGIC_TASK_GEN_CADENCE_TICKS: u64 = 30;

/// Regenerate strategic tasks when SettlementIntent changes or dirty/cadence.
///
/// Never assigns workers.
pub fn step_settlement_strategic_task_generation(
    world: &mut WorldData,
    catalog: &StrategicTaskTemplateCatalog,
    simulation_tick: u64,
) -> u32 {
    let settlement_ids: Vec<SettlementId> = world
        .settlement_state_store()
        .iter()
        .map(|(id, _)| *id)
        .collect();

    let mut generated = 0u32;
    for settlement_id in settlement_ids {
        let Some(intent_plan) = world.settlement_intent_store().get(settlement_id).cloned() else {
            // No plan yet (e.g. post-load): drop Available strategic tasks so they do not linger.
            if world.strategic_task_generation_store().is_dirty(settlement_id)
                || world.strategic_task_generation_store().get(settlement_id).is_some()
            {
                let cancelled = cancel_available_strategic_tasks(world, settlement_id);
                if cancelled > 0 || world.strategic_task_generation_store().get(settlement_id).is_some()
                {
                    world.strategic_task_generation_store_mut().insert(
                        super::report::StrategicTaskGenerationReport {
                            settlement_id,
                            generated_tick: simulation_tick,
                            source_intent_tick: 0,
                            emissions: Vec::new(),
                            cancelled_task_ids: Vec::new(),
                            diagnostics: vec![format!(
                                "no intent plan; cancelled {cancelled} available strategic tasks"
                            )],
                        },
                    );
                    generated += 1;
                }
            }
            continue;
        };

        let due_by_dirty = world.strategic_task_generation_store().is_dirty(settlement_id);
        let due_by_intent = world
            .strategic_task_generation_store()
            .get(settlement_id)
            .map(|prev| prev.source_intent_tick != intent_plan.planned_tick)
            .unwrap_or(true);
        let due_by_cadence = match world.strategic_task_generation_store().get(settlement_id) {
            None => true,
            Some(prev) => {
                simulation_tick.saturating_sub(prev.generated_tick)
                    >= STRATEGIC_TASK_GEN_CADENCE_TICKS
            }
        };
        if !due_by_dirty && !due_by_intent && !due_by_cadence {
            continue;
        }

        let mut ctx = StrategicTaskGenContext {
            world,
            catalog,
            intent_plan: &intent_plan,
            simulation_tick,
        };
        let report = generate_strategic_tasks_for_settlement(&mut ctx);
        world.strategic_task_generation_store_mut().insert(report);
        generated += 1;
    }
    generated
}

fn cancel_available_strategic_tasks(world: &mut WorldData, settlement_id: SettlementId) -> u32 {
    let settlement_raw = settlement_id.raw();
    let task_ids = world.task_store().sorted_task_ids();
    let mut cancelled = 0u32;
    for task_id in task_ids {
        let Some(task) = world.task_store().get(task_id) else {
            continue;
        };
        let Some(origin) = &task.strategic else {
            continue;
        };
        if origin.settlement_id != settlement_raw {
            continue;
        }
        if task.state != crate::world::task::TaskState::Available {
            continue;
        }
        world.task_store_mut().remove_task(task_id);
        cancelled += 1;
    }
    cancelled
}

pub fn generate_strategic_tasks_now(
    world: &mut WorldData,
    catalog: &StrategicTaskTemplateCatalog,
    settlement_id: SettlementId,
    simulation_tick: u64,
) {
    let Some(intent_plan) = world.settlement_intent_store().get(settlement_id).cloned() else {
        return;
    };
    let mut ctx = StrategicTaskGenContext {
        world,
        catalog,
        intent_plan: &intent_plan,
        simulation_tick,
    };
    let report = generate_strategic_tasks_for_settlement(&mut ctx);
    world.strategic_task_generation_store_mut().insert(report);
}
