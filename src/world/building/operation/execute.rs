//! Generic production execution — inventory mutations on cycle completion (EP5).

use std::collections::HashMap;

use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::building::operational_efficiency::OperationalLimitingFactor;
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryError, InventoryId, InventoryRecord, InventoryStore,
    ItemInstanceStore, PlacedInventoryEntry, can_place_entry, consume_stack_item, count_stack_item,
    first_fit_position, place_stack_first_fit,
};
use crate::world::logistics::available_stack_quantity;
use crate::world::operation::{OperationDefinition, OperationOutputDefinition};
use crate::world::{ItemDefinitionId, WorldData};

/// Readiness / execution failure mapped to production blocking factors (EP5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionExecutionFailure {
    InvalidOperation,
    MissingInventoryBinding(BuildingInventoryBindingId),
    MissingInventory {
        binding_id: BuildingInventoryBindingId,
        inventory_id: InventoryId,
    },
    MissingInput {
        item_id: ItemDefinitionId,
        required: u32,
        available: u32,
    },
    InputReserved {
        item_id: ItemDefinitionId,
        required: u32,
        available_unreserved: u32,
        physical: u32,
    },
    OutputFull {
        item_id: ItemDefinitionId,
        binding_id: BuildingInventoryBindingId,
    },
}

impl ProductionExecutionFailure {
    pub fn limiting_factor(&self) -> OperationalLimitingFactor {
        match self {
            Self::InvalidOperation => OperationalLimitingFactor::InvalidOperation,
            Self::MissingInventoryBinding(_) => OperationalLimitingFactor::InvalidInventoryBinding,
            Self::MissingInventory { .. } => OperationalLimitingFactor::MissingInventory,
            Self::MissingInput { .. } => OperationalLimitingFactor::MissingInput,
            Self::InputReserved { .. } => OperationalLimitingFactor::InputReserved,
            Self::OutputFull { .. } => OperationalLimitingFactor::OutputBlocked,
        }
    }
}

/// Resolved operation input for inspection (EP5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProductionInput {
    pub binding_id: BuildingInventoryBindingId,
    pub inventory_id: InventoryId,
    pub item_id: ItemDefinitionId,
    pub required: u32,
    pub available: u32,
    pub physical: u32,
    pub reserved: u32,
}

/// Resolved operation output for inspection (EP5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProductionOutput {
    pub binding_id: BuildingInventoryBindingId,
    pub inventory_id: InventoryId,
    pub item_id: ItemDefinitionId,
    pub quantity: u32,
    pub can_accept: bool,
}

/// Authoritative execution readiness probe (EP5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProductionExecutionAssessment {
    pub inputs: Vec<ResolvedProductionInput>,
    pub outputs: Vec<ResolvedProductionOutput>,
    pub blocking: Option<ProductionExecutionFailure>,
}

impl ProductionExecutionAssessment {
    pub fn blocking_label(&self) -> Option<&'static str> {
        self.blocking
            .as_ref()
            .map(|failure| failure.limiting_factor().label())
    }
}

struct AggregatedInput {
    binding_id: BuildingInventoryBindingId,
    inventory_id: InventoryId,
    item_id: ItemDefinitionId,
    quantity: u32,
}

struct ResolvedItemOutput {
    binding_id: BuildingInventoryBindingId,
    inventory_id: InventoryId,
    item_id: ItemDefinitionId,
    quantity: u32,
}

struct InventoryRollback {
    snapshots: HashMap<InventoryId, InventoryRecord>,
}

impl InventoryRollback {
    fn snapshot(&mut self, store: &InventoryStore, inventory_id: InventoryId) {
        if self.snapshots.contains_key(&inventory_id) {
            return;
        }
        if let Some(record) = store.get(inventory_id) {
            self.snapshots.insert(inventory_id, record.clone());
        }
    }

    fn restore(self, store: &mut InventoryStore) {
        for (inventory_id, record) in self.snapshots {
            if let Some(slot) = store.get_mut(inventory_id) {
                *slot = record;
            }
        }
    }
}

/// Assess whether a completed production cycle can execute without mutating state (EP5).
pub fn assess_production_execution(
    world: &WorldData,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    building_id: crate::world::BuildingId,
    operation: &OperationDefinition,
    building: &BuildingDefinition,
) -> ProductionExecutionAssessment {
    let mut assessment = ProductionExecutionAssessment::default();
    match plan_execution(world, operation, building_id) {
        Ok(plan) => {
            assessment.inputs = plan
                .inputs
                .iter()
                .map(|input| {
                    let physical = world
                        .inventory_store()
                        .get(input.inventory_id)
                        .map(|record| count_stack_item(record, &input.item_id))
                        .unwrap_or(0);
                    let available = available_stack_quantity(
                        world.inventory_store(),
                        world.inventory_reservation_store(),
                        input.inventory_id,
                        &input.item_id,
                    );
                    ResolvedProductionInput {
                        binding_id: input.binding_id.clone(),
                        inventory_id: input.inventory_id,
                        item_id: input.item_id.clone(),
                        required: input.quantity,
                        available,
                        physical,
                        reserved: physical.saturating_sub(available),
                    }
                })
                .collect();

            let mut sims: HashMap<InventoryId, InventoryRecord> = HashMap::new();
            for output in &plan.outputs {
                let can_accept = simulate_output_placement(
                    world,
                    inventory_ctx,
                    &mut sims,
                    output,
                );
                assessment.outputs.push(ResolvedProductionOutput {
                    binding_id: output.binding_id.clone(),
                    inventory_id: output.inventory_id,
                    item_id: output.item_id.clone(),
                    quantity: output.quantity,
                    can_accept,
                });
            }

            if let Err(failure) = validate_plan(world, inventory_ctx, &plan) {
                assessment.blocking = Some(failure);
            }
        }
        Err(failure) => {
            assessment.blocking = Some(failure);
        }
    }
    let _ = building;
    assessment
}

/// Execute one completed production cycle atomically (EP5).
pub fn execute_production_cycle(
    world: &mut WorldData,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    building_id: crate::world::BuildingId,
    operation: &OperationDefinition,
    building: &BuildingDefinition,
) -> Result<(), OperationalLimitingFactor> {
    let plan = plan_execution(world, operation, building_id)
        .map_err(|failure| failure.limiting_factor())?;
    validate_plan(world, inventory_ctx, &plan).map_err(|failure| failure.limiting_factor())?;

    let mut rollback = InventoryRollback {
        snapshots: HashMap::new(),
    };
    let _ = building;

    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    for input in &plan.inputs {
        rollback.snapshot(inventory_store, input.inventory_id);
    }
    for output in &plan.outputs {
        rollback.snapshot(inventory_store, output.inventory_id);
    }

    for input in &plan.inputs {
        let consumed = consume_stack_item(
            inventory_store,
            instance_store,
            inventory_ctx,
            input.inventory_id,
            &input.item_id,
            input.quantity,
        )
        .map_err(|_| OperationalLimitingFactor::MissingInput)?;
        if consumed != input.quantity {
            rollback.restore(inventory_store);
            return Err(OperationalLimitingFactor::MissingInput);
        }
    }

    for output in &plan.outputs {
        if place_stack_quantity_first_fit(
            inventory_store,
            instance_store,
            inventory_ctx,
            output.inventory_id,
            output.item_id.clone(),
            output.quantity,
        )
        .is_err()
        {
            rollback.restore(inventory_store);
            return Err(OperationalLimitingFactor::OutputBlocked);
        }
    }

    Ok(())
}

struct ExecutionPlan {
    inputs: Vec<AggregatedInput>,
    outputs: Vec<ResolvedItemOutput>,
}

fn plan_execution(
    world: &WorldData,
    operation: &OperationDefinition,
    building_id: crate::world::BuildingId,
) -> Result<ExecutionPlan, ProductionExecutionFailure> {
    let binding_store = world.building_inventory_binding_store();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    for input in &operation.inputs {
        let Some(binding_id) = input.source_binding.as_ref() else {
            if input.quantity > 0 {
                return Err(ProductionExecutionFailure::InvalidOperation);
            }
            continue;
        };
        let inventory_id = resolve_binding_inventory(
            world,
            binding_store,
            building_id,
            binding_id,
        )?;
        inputs.push(AggregatedInput {
            binding_id: binding_id.clone(),
            inventory_id,
            item_id: input.item_id.clone(),
            quantity: input.quantity,
        });
    }

    for output in &operation.outputs {
        let OperationOutputDefinition::Item {
            item_id,
            quantity,
            destination_binding,
        } = output
        else {
            continue;
        };
        let Some(binding_id) = destination_binding.as_ref() else {
            if *quantity > 0 {
                return Err(ProductionExecutionFailure::InvalidOperation);
            }
            continue;
        };
        let inventory_id = resolve_binding_inventory(
            world,
            binding_store,
            building_id,
            binding_id,
        )?;
        outputs.push(ResolvedItemOutput {
            binding_id: binding_id.clone(),
            inventory_id,
            item_id: item_id.clone(),
            quantity: *quantity,
        });
    }

    Ok(ExecutionPlan {
        inputs: aggregate_inputs(inputs),
        outputs,
    })
}

fn aggregate_inputs(inputs: Vec<AggregatedInput>) -> Vec<AggregatedInput> {
    let mut merged: HashMap<(InventoryId, ItemDefinitionId), AggregatedInput> = HashMap::new();
    for input in inputs {
        let key = (input.inventory_id, input.item_id.clone());
        merged
            .entry(key)
            .and_modify(|existing| {
                existing.quantity = existing.quantity.saturating_add(input.quantity);
            })
            .or_insert(input);
    }
    merged.into_values().collect()
}

fn resolve_binding_inventory(
    world: &WorldData,
    binding_store: &crate::world::building::inventory_binding::BuildingInventoryBindingStore,
    building_id: crate::world::BuildingId,
    binding_id: &BuildingInventoryBindingId,
) -> Result<InventoryId, ProductionExecutionFailure> {
    let Some(inventory_id) = binding_store.resolve_inventory(building_id, binding_id) else {
        return Err(ProductionExecutionFailure::MissingInventoryBinding(
            binding_id.clone(),
        ));
    };
    if world.inventory_store().get(inventory_id).is_none() {
        return Err(ProductionExecutionFailure::MissingInventory {
            binding_id: binding_id.clone(),
            inventory_id,
        });
    }
    Ok(inventory_id)
}

fn validate_plan(
    world: &WorldData,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    plan: &ExecutionPlan,
) -> Result<(), ProductionExecutionFailure> {
    for input in &plan.inputs {
        let physical = world
            .inventory_store()
            .get(input.inventory_id)
            .map(|record| count_stack_item(record, &input.item_id))
            .unwrap_or(0);
        let available = available_stack_quantity(
            world.inventory_store(),
            world.inventory_reservation_store(),
            input.inventory_id,
            &input.item_id,
        );
        if available < input.quantity {
            if physical >= input.quantity {
                return Err(ProductionExecutionFailure::InputReserved {
                    item_id: input.item_id.clone(),
                    required: input.quantity,
                    available_unreserved: available,
                    physical,
                });
            }
            return Err(ProductionExecutionFailure::MissingInput {
                item_id: input.item_id.clone(),
                required: input.quantity,
                available,
            });
        }
    }

    let mut sims: HashMap<InventoryId, InventoryRecord> = HashMap::new();
    for output in &plan.outputs {
        if !simulate_output_placement(world, inventory_ctx, &mut sims, output) {
            return Err(ProductionExecutionFailure::OutputFull {
                item_id: output.item_id.clone(),
                binding_id: output.binding_id.clone(),
            });
        }
    }

    Ok(())
}

fn simulate_output_placement(
    world: &WorldData,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    sims: &mut HashMap<InventoryId, InventoryRecord>,
    output: &ResolvedItemOutput,
) -> bool {
    let sim = sims.entry(output.inventory_id).or_insert_with(|| {
        world
            .inventory_store()
            .get(output.inventory_id)
            .expect("inventory validated during planning")
            .clone()
    });
    place_stack_quantity_on_record(sim, inventory_ctx, &output.item_id, output.quantity)
}

fn place_stack_quantity_on_record(
    record: &mut InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    item_id: &ItemDefinitionId,
    mut quantity: u32,
) -> bool {
    if quantity == 0 {
        return true;
    }
    let Ok(item) = ctx.require_item(item_id) else {
        return false;
    };
    if item.unique_instance_required || !item.stackable {
        return false;
    }
    let Ok(limit) = ctx.stack_limit_for(item, record.profile_id()) else {
        return false;
    };

    while quantity > 0 {
        let chunk = quantity.min(limit);
        let Ok((anchor_x, anchor_y)) =
            first_fit_position(record, item.grid_width, item.grid_height)
        else {
            return false;
        };
        let entry = PlacedInventoryEntry::stack(anchor_x, anchor_y, item_id.clone(), chunk);
        if can_place_entry(record, &entry, item_id, None, ctx).is_err() {
            return false;
        }
        record.placed_entries_mut().push(entry);
        if record
            .rebuild_derived(ctx, |id| Err(InventoryError::ItemInstanceNotFound(id)))
            .is_err()
        {
            return false;
        }
        quantity -= chunk;
    }
    true
}

fn place_stack_quantity_first_fit(
    inventory_store: &mut InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    item_id: ItemDefinitionId,
    mut quantity: u32,
) -> Result<(), InventoryError> {
    if quantity == 0 {
        return Ok(());
    }
    let profile_id = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .profile_id()
        .clone();
    while quantity > 0 {
        let item = ctx.require_item(&item_id)?;
        let limit = ctx.stack_limit_for(item, &profile_id)?;
        let chunk = quantity.min(limit);
        place_stack_first_fit(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            item_id.clone(),
            chunk,
        )?;
        quantity -= chunk;
    }
    Ok(())
}
