//! Self-maintenance simulation step (ADR-134).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::ItemCatalog;
use crate::world::movement::feel::start_unit_move_to;
use crate::world::task::{TaskState, TaskType, release_unit_task_to_marketplace};
use crate::world::unit::catalog::UnitCatalog;
use crate::world::unit::{CombatState, UnitId, UnitState, unit_can_execute_actions};
use crate::world::{
    BuildingInteractionProfileCatalog, DoodadCatalog, FootprintCatalog, NavigationConfig,
    PassabilityCatalogs, WorldData,
};

use super::food::{
    eat_one_from_inventory, edible_to_source, select_food_source, unit_near_food_source,
};
use super::nutrition::{
    HungerStage, NutritionProfile, UnitNutritionState, apply_nutrition_decay, evaluate_hunger_stage,
};
use super::state::{FoodSourceRef, SelfMaintenanceActivity, UnitSelfMaintenanceState};

/// Whether the unit is in active combat (hunger must not interrupt).
pub fn unit_in_active_combat(combat_state: &CombatState) -> bool {
    matches!(
        combat_state,
        CombatState::Attacking { .. }
            | CombatState::Chasing { .. }
            | CombatState::AttackMoving { .. }
    )
}

/// Initialize nutrition for a newly spawned unit from its definition.
pub fn initialize_unit_nutrition(
    nutrition: &mut UnitNutritionState,
    definition: &crate::world::unit::catalog::UnitDefinition,
) {
    if let Some(profile) = NutritionProfile::from_definition(definition) {
        *nutrition = UnitNutritionState::full(profile.max);
    } else {
        *nutrition = UnitNutritionState::default();
    }
}

pub struct SelfMaintenanceContext<'a> {
    pub world: &'a mut WorldData,
    pub unit_catalog: &'a UnitCatalog,
    pub building_catalog: &'a BuildingCatalog,
    pub interaction_catalog: &'a BuildingInteractionProfileCatalog,
    pub item_catalog: &'a ItemCatalog,
    pub inventory_ctx: &'a InventoryCatalogCtx<'a>,
    pub passability: PassabilityCatalogs<'a>,
    pub nav_config: &'a NavigationConfig,
}

/// Decay nutrition for all living units using the shared authored consumption rate.
pub fn step_unit_nutrition_decay(ctx: &mut SelfMaintenanceContext<'_>) {
    let unit_ids = ctx.world.sorted_unit_ids();
    for unit_id in unit_ids {
        let Some(definition) = ctx
            .world
            .get_unit(unit_id)
            .and_then(|record| ctx.unit_catalog.get(&record.definition_id))
        else {
            continue;
        };
        let Some(profile) = NutritionProfile::from_definition(definition) else {
            continue;
        };
        ctx.world.mutate_unit(unit_id, |record| {
            apply_nutrition_decay(&mut record.nutrition, &profile);
        });
    }
}

/// Pre-work hunger decisions: interrupt critical work, begin opportunistic seeking.
pub fn step_unit_self_maintenance_pre_work(ctx: &mut SelfMaintenanceContext<'_>) {
    let unit_ids = ctx.world.sorted_unit_ids();
    for unit_id in unit_ids {
        if !unit_can_execute_actions(ctx.world, unit_id) {
            continue;
        }
        let snapshot = match ctx.world.get_unit(unit_id) {
            Some(record) => record.clone(),
            None => continue,
        };
        let Some(definition) = ctx.unit_catalog.get(&snapshot.definition_id) else {
            continue;
        };
        let Some(profile) = NutritionProfile::from_definition(definition) else {
            continue;
        };
        if unit_in_active_combat(&snapshot.combat_state) {
            continue;
        }
        let stage = evaluate_hunger_stage(snapshot.nutrition.current, &profile);
        if stage == HungerStage::Fed {
            ctx.world.mutate_unit(unit_id, |record| {
                if record.self_maintenance.is_seeking_or_eating() {
                    record.self_maintenance.clear();
                }
            });
            continue;
        }

        match &snapshot.self_maintenance.activity {
            SelfMaintenanceActivity::Eating { .. } => {
                try_continue_eating(ctx, unit_id, &profile);
                continue;
            }
            SelfMaintenanceActivity::SeekingFood { destination, .. } => {
                if unit_near_food_source(
                    snapshot.placement.position,
                    *destination,
                    ctx.world.layout(),
                ) {
                    begin_eating_at_destination(ctx, unit_id, stage);
                }
                continue;
            }
            SelfMaintenanceActivity::None => {}
        }

        if stage == HungerStage::Critical && matches!(snapshot.state, UnitState::Working { .. }) {
            let abandon_work = ctx
                .world
                .task_store()
                .unit_task_id(unit_id)
                .and_then(|task_id| ctx.world.task_store().get(task_id))
                .is_none_or(|task| {
                    !matches!(
                        (task.task_type, task.state),
                        (TaskType::OperateWorkstation, TaskState::InProgress)
                    )
                });
            if abandon_work {
                let mut events = Vec::new();
                release_unit_task_to_marketplace(ctx.world, unit_id, &mut events);
                let _ = events;
            }
        }

        let may_begin_seek = match stage {
            HungerStage::Critical => true,
            HungerStage::Normal => {
                matches!(snapshot.state, UnitState::Idle)
                    && ctx.world.task_store().unit_task_id(unit_id).is_none()
            }
            HungerStage::Fed => false,
        };
        if !may_begin_seek {
            continue;
        }

        let settlement_id = snapshot.settlement_id;
        let Some(edible) = select_food_source(
            ctx.world,
            ctx.building_catalog,
            ctx.interaction_catalog,
            ctx.item_catalog,
            unit_id,
            settlement_id,
        ) else {
            continue;
        };

        if edible.building_id.is_none() {
            begin_eating_own_inventory(ctx, unit_id, &edible, stage, &profile);
            continue;
        }

        let destination = edible.interaction_position;
        let source = edible_to_source(&edible);
        if unit_near_food_source(snapshot.placement.position, destination, ctx.world.layout()) {
            ctx.world.mutate_unit(unit_id, |record| {
                record.self_maintenance.activity =
                    SelfMaintenanceActivity::Eating { source, stage };
            });
            try_continue_eating(ctx, unit_id, &profile);
            continue;
        }

        if start_unit_move_to(
            ctx.world,
            ctx.unit_catalog,
            ctx.passability,
            ctx.nav_config,
            unit_id,
            destination,
        )
        .is_ok()
        {
            ctx.world.mutate_unit(unit_id, |record| {
                record.self_maintenance.activity = SelfMaintenanceActivity::SeekingFood {
                    source,
                    destination,
                    stage,
                };
            });
        }
    }
}

/// Post-movement: transition seekers to eating and consume food until full or exhausted.
pub fn step_unit_self_maintenance_post_movement(ctx: &mut SelfMaintenanceContext<'_>) {
    let unit_ids = ctx.world.sorted_unit_ids();
    for unit_id in unit_ids {
        if !unit_can_execute_actions(ctx.world, unit_id) {
            continue;
        }
        let snapshot = match ctx.world.get_unit(unit_id) {
            Some(record) => record.clone(),
            None => continue,
        };
        let Some(definition) = ctx.unit_catalog.get(&snapshot.definition_id) else {
            continue;
        };
        let Some(profile) = NutritionProfile::from_definition(definition) else {
            continue;
        };
        if unit_in_active_combat(&snapshot.combat_state) {
            continue;
        }

        match &snapshot.self_maintenance.activity {
            SelfMaintenanceActivity::SeekingFood {
                destination, stage, ..
            } => {
                if matches!(snapshot.state, UnitState::Idle)
                    && unit_near_food_source(
                        snapshot.placement.position,
                        *destination,
                        ctx.world.layout(),
                    )
                {
                    begin_eating_at_destination(ctx, unit_id, *stage);
                    try_continue_eating(ctx, unit_id, &profile);
                }
            }
            SelfMaintenanceActivity::Eating { .. } => {
                try_continue_eating(ctx, unit_id, &profile);
            }
            SelfMaintenanceActivity::None => {}
        }
    }
}

/// Whether SA7 should skip assigning new work to this unit due to hunger.
///
/// Hungry or critically hungry idle workers defer to food when an accessible source exists.
/// When no food is reachable, work claims remain allowed so units can still perform
/// food-producing labor (for example farm harvest).
pub fn hunger_prevents_work_claim(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    item_catalog: &ItemCatalog,
    unit_id: UnitId,
) -> bool {
    let Some(record) = world.get_unit(unit_id) else {
        return false;
    };
    if unit_in_active_combat(&record.combat_state) {
        return false;
    }
    let Some(definition) = unit_catalog.get(&record.definition_id) else {
        return false;
    };
    let Some(profile) = NutritionProfile::from_definition(definition) else {
        return false;
    };
    if record.self_maintenance.is_seeking_or_eating() {
        return true;
    }
    if !matches!(record.state, UnitState::Idle)
        || world.task_store().unit_task_id(unit_id).is_some()
    {
        return false;
    }
    let stage = evaluate_hunger_stage(record.nutrition.current, &profile);
    if stage == HungerStage::Fed {
        return false;
    }
    select_food_source(
        world,
        building_catalog,
        interaction_catalog,
        item_catalog,
        unit_id,
        record.settlement_id,
    )
    .is_some()
}

fn begin_eating_at_destination(
    ctx: &mut SelfMaintenanceContext<'_>,
    unit_id: UnitId,
    stage: HungerStage,
) {
    let activity = ctx
        .world
        .get_unit(unit_id)
        .map(|record| record.self_maintenance.activity.clone());
    let Some(SelfMaintenanceActivity::SeekingFood { source, .. }) = activity else {
        return;
    };
    ctx.world.mutate_unit(unit_id, |record| {
        record.self_maintenance.activity = SelfMaintenanceActivity::Eating { source, stage };
    });
}

fn begin_eating_own_inventory(
    ctx: &mut SelfMaintenanceContext<'_>,
    unit_id: UnitId,
    edible: &super::food::EdibleStack,
    stage: HungerStage,
    profile: &NutritionProfile,
) {
    let source = edible_to_source(edible);
    ctx.world.mutate_unit(unit_id, |record| {
        record.self_maintenance.activity = SelfMaintenanceActivity::Eating { source, stage };
    });
    try_continue_eating(ctx, unit_id, profile);
}

fn try_continue_eating(
    ctx: &mut SelfMaintenanceContext<'_>,
    unit_id: UnitId,
    profile: &NutritionProfile,
) {
    loop {
        let snapshot = match ctx.world.get_unit(unit_id) {
            Some(record) => record.clone(),
            None => return,
        };
        if evaluate_hunger_stage(snapshot.nutrition.current, profile) == HungerStage::Fed {
            ctx.world.mutate_unit(unit_id, |record| {
                record.self_maintenance.clear();
            });
            return;
        }

        let activity = snapshot.self_maintenance.activity.clone();
        let SelfMaintenanceActivity::Eating { source, .. } = activity else {
            return;
        };

        let edible = match resolve_eating_source(ctx, unit_id, snapshot.settlement_id, &source) {
            Some(edible) => edible,
            None => {
                ctx.world.mutate_unit(unit_id, |record| {
                    record.self_maintenance.clear();
                });
                return;
            }
        };

        let ate = {
            let mut nutrition = snapshot.nutrition;
            let success = eat_one_from_inventory(
                ctx.world,
                ctx.inventory_ctx,
                unit_id,
                &mut nutrition,
                profile,
                edible.inventory_id,
                &edible.item_definition_id,
                ctx.item_catalog,
            );
            if success {
                ctx.world.mutate_unit(unit_id, |record| {
                    record.nutrition = nutrition;
                });
            }
            success
        };

        if !ate {
            ctx.world.mutate_unit(unit_id, |record| {
                record.self_maintenance.clear();
            });
            return;
        }

        let after = match ctx.world.get_unit(unit_id) {
            Some(record) => record,
            None => return,
        };
        if evaluate_hunger_stage(after.nutrition.current, profile) == HungerStage::Fed {
            ctx.world.mutate_unit(unit_id, |record| {
                record.self_maintenance.clear();
            });
            return;
        }
    }
}

fn resolve_eating_source(
    ctx: &SelfMaintenanceContext<'_>,
    unit_id: UnitId,
    settlement_id: Option<crate::world::settlement::SettlementId>,
    source: &FoodSourceRef,
) -> Option<super::food::EdibleStack> {
    match source {
        FoodSourceRef::OwnInventory { inventory_id } => {
            let unit = ctx.world.get_unit(unit_id)?;
            super::food::find_edible_in_inventory(
                ctx.world,
                ctx.item_catalog,
                *inventory_id,
                unit.placement.position,
                ctx.world.layout(),
            )
        }
        FoodSourceRef::SettlementStorage {
            inventory_id,
            building_id,
        } => {
            let unit = ctx.world.get_unit(unit_id)?;
            if settlement_id.is_none() {
                return None;
            }
            let settlement_id = settlement_id?;
            let building = ctx.world.get_building(*building_id)?;
            if building.settlement_id != Some(settlement_id) {
                return None;
            }
            let inventory = ctx.world.inventory_store().get(*inventory_id)?;
            let interaction_pos = super::food::building_food_interaction_position(
                ctx.world,
                *building_id,
                ctx.interaction_catalog,
                ctx.world.layout(),
            );
            for (entry_index, entry) in inventory.placed_entries().iter().enumerate() {
                if let crate::world::inventory::InventoryEntryContents::Stack {
                    item_definition_id,
                    quantity,
                } = &entry.contents
                {
                    if *quantity > 0
                        && super::food::is_edible_food(ctx.item_catalog, item_definition_id)
                    {
                        let nutrition = ctx.item_catalog.get(item_definition_id)?.nutrition;
                        return Some(super::food::EdibleStack {
                            inventory_id: *inventory_id,
                            building_id: Some(*building_id),
                            item_definition_id: item_definition_id.clone(),
                            entry_index,
                            nutrition,
                            interaction_position: interaction_pos,
                            distance_sq: 0.0,
                        });
                    }
                }
            }
            let _ = unit;
            None
        }
    }
}
