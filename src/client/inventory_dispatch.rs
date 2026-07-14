//! Authoritative inventory intent dispatch (ADR-092 I6).

use bevy::prelude::*;

use crate::ui::gameplay::InventoryUiError;
use crate::ui::gameplay::InventoryUiState;
use crate::world::{
    BuildingCatalog, BuildingId, BuildingInteractionProfileCatalog, CorpseId, EntryIndex,
    InventoryAccessResult, InventoryCatalogCtx, InventoryId, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, ItemPileId, ItemPileSettings, TransferPlacementPolicy,
    TreasuryAccessPolicy, UnitId, WorldData, auto_sort, can_unit_access_building_inventory,
    can_unit_access_inventory, count_physical_gold, deposit_gold, drop_unit_inventory_entry,
    half_stack_quantity, loot_corpse_entry, move_entry, pickup_pile_into_inventory,
    transfer_entry_full, transfer_half, transfer_one,
};

use super::inventory_intent::{
    DepositGoldAmount, InventoryIntent, InventoryIntentQueue, InventoryIntentStatus,
    InventoryOpenMode, entry_revision_for_inventory,
};

/// Dispatch inventory intents against authoritative world data.
pub fn dispatch_inventory_intents(
    mut queue: ResMut<InventoryIntentQueue>,
    mut ui: ResMut<InventoryUiState>,
    mut world: ResMut<WorldData>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    building_catalog: Res<BuildingCatalog>,
    interaction_catalog: Res<BuildingInteractionProfileCatalog>,
    pile_settings: Res<ItemPileSettings>,
    simulation: Res<crate::simulation::SimulationControlState>,
) {
    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let tick = simulation.current_tick;
    for intent in queue.drain() {
        let status = dispatch_one(
            &intent,
            &mut ui,
            &mut world,
            &ctx,
            &building_catalog,
            &interaction_catalog,
            &pile_settings,
            tick,
        );
        if status == InventoryIntentStatus::Rejected {
            ui.invalidate_drag();
        }
        let _ = status;
    }
    reconcile_open_inventories(&mut ui, &world);
}

fn dispatch_one(
    intent: &InventoryIntent,
    ui: &mut InventoryUiState,
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    pile_settings: &ItemPileSettings,
    tick: u64,
) -> InventoryIntentStatus {
    match intent {
        InventoryIntent::Open(mode) => {
            apply_open_mode(ui, world, mode);
            InventoryIntentStatus::Applied
        }
        InventoryIntent::Close => {
            ui.close();
            InventoryIntentStatus::Applied
        }
        InventoryIntent::MoveEntry {
            inventory_id,
            entry_index,
            anchor_x,
            anchor_y,
            entry_revision,
        } => {
            if !revision_matches(world, *inventory_id, *entry_index, *entry_revision) {
                ui.feedback_message = InventoryUiError::ItemChanged.message();
                return InventoryIntentStatus::Rejected;
            }
            if let Some(actor) = ui.actor_unit_id {
                if !can_access_inventory(world, building_catalog, actor, *inventory_id, ui) {
                    ui.feedback_message = InventoryUiError::AccessDenied.message();
                    return InventoryIntentStatus::Rejected;
                }
            }
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            match move_entry(
                inventory_store,
                instance_store,
                ctx,
                *inventory_id,
                *entry_index,
                *anchor_x,
                *anchor_y,
            ) {
                Ok(()) => {
                    ui.feedback_message.clear();
                    ui.invalidate_drag();
                    InventoryIntentStatus::Applied
                }
                Err(error) => {
                    ui.feedback_message = InventoryUiError::from_inventory(error).message();
                    InventoryIntentStatus::Rejected
                }
            }
        }
        InventoryIntent::TransferFull {
            source_inventory_id,
            source_entry_index,
            destination_inventory_id,
            entry_revision,
        } => transfer_with_policy(
            ui,
            world,
            ctx,
            building_catalog,
            *source_inventory_id,
            *source_entry_index,
            *destination_inventory_id,
            *entry_revision,
            TransferMode::Full,
        ),
        InventoryIntent::TransferOne {
            source_inventory_id,
            source_entry_index,
            destination_inventory_id,
            entry_revision,
        } => transfer_with_policy(
            ui,
            world,
            ctx,
            building_catalog,
            *source_inventory_id,
            *source_entry_index,
            *destination_inventory_id,
            *entry_revision,
            TransferMode::One,
        ),
        InventoryIntent::TransferHalf {
            source_inventory_id,
            source_entry_index,
            destination_inventory_id,
            entry_revision,
        } => transfer_with_policy(
            ui,
            world,
            ctx,
            building_catalog,
            *source_inventory_id,
            *source_entry_index,
            *destination_inventory_id,
            *entry_revision,
            TransferMode::Half,
        ),
        InventoryIntent::TransferToCell {
            source_inventory_id,
            source_entry_index,
            destination_inventory_id,
            anchor_x,
            anchor_y,
            entry_revision,
        } => {
            if !revision_matches(
                world,
                *source_inventory_id,
                *source_entry_index,
                *entry_revision,
            ) {
                ui.feedback_message = InventoryUiError::ItemChanged.message();
                return InventoryIntentStatus::Rejected;
            }
            if let Some(actor) = ui.actor_unit_id {
                if !can_access_pair(
                    world,
                    building_catalog,
                    actor,
                    *source_inventory_id,
                    *destination_inventory_id,
                    ui,
                ) {
                    ui.feedback_message = InventoryUiError::AccessDenied.message();
                    return InventoryIntentStatus::Rejected;
                }
            }
            let policy = if source_inventory_id == destination_inventory_id {
                TransferPlacementPolicy::ExactCell {
                    x: *anchor_x,
                    y: *anchor_y,
                }
            } else {
                TransferPlacementPolicy::ExactCell {
                    x: *anchor_x,
                    y: *anchor_y,
                }
            };
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            match transfer_entry_full(
                inventory_store,
                instance_store,
                ctx,
                *source_inventory_id,
                *source_entry_index,
                *destination_inventory_id,
                policy,
            ) {
                Ok(_) => {
                    ui.feedback_message.clear();
                    ui.invalidate_drag();
                    InventoryIntentStatus::Applied
                }
                Err(error) => {
                    ui.feedback_message = InventoryUiError::from_transfer(error).message();
                    InventoryIntentStatus::Rejected
                }
            }
        }
        InventoryIntent::AutoSort { inventory_id } => {
            if let Some(actor) = ui.actor_unit_id {
                if !can_access_inventory(world, building_catalog, actor, *inventory_id, ui) {
                    ui.feedback_message = InventoryUiError::AccessDenied.message();
                    return InventoryIntentStatus::Rejected;
                }
            }
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            match auto_sort(inventory_store, instance_store, ctx, *inventory_id) {
                Ok(_) => {
                    ui.feedback_message = "Sorted.".into();
                    InventoryIntentStatus::Applied
                }
                Err(error) => {
                    ui.feedback_message = InventoryUiError::from_inventory(error).message();
                    InventoryIntentStatus::Rejected
                }
            }
        }
        InventoryIntent::DropEntry {
            inventory_id: _,
            entry_index,
            actor_unit_id,
            entry_revision,
        } => {
            let unit = match world.get_unit(*actor_unit_id) {
                Some(u) => u.clone(),
                None => {
                    ui.feedback_message = InventoryUiError::AccessDenied.message();
                    return InventoryIntentStatus::Rejected;
                }
            };
            let Some(inventory_id) = unit.inventory_id else {
                ui.feedback_message = InventoryUiError::InventoryClosed.message();
                return InventoryIntentStatus::Rejected;
            };
            if !revision_matches(world, inventory_id, *entry_index, *entry_revision) {
                ui.feedback_message = InventoryUiError::ItemChanged.message();
                return InventoryIntentStatus::Rejected;
            }
            match drop_unit_inventory_entry(
                world,
                ctx,
                pile_settings,
                *actor_unit_id,
                *entry_index,
                None,
                tick,
            ) {
                Ok(_) => {
                    ui.feedback_message = "Dropped.".into();
                    ui.invalidate_drag();
                    InventoryIntentStatus::Applied
                }
                Err(error) => {
                    ui.feedback_message = InventoryUiError::from_pile(error).message();
                    InventoryIntentStatus::Rejected
                }
            }
        }
        InventoryIntent::PickupPile {
            pile_id,
            actor_unit_id,
            quantity,
        } => {
            let Some(unit) = world.get_unit(*actor_unit_id) else {
                ui.feedback_message = InventoryUiError::AccessDenied.message();
                return InventoryIntentStatus::Rejected;
            };
            let Some(inventory_id) = unit.inventory_id else {
                ui.feedback_message = InventoryUiError::NoRoom.message();
                return InventoryIntentStatus::Rejected;
            };
            match pickup_pile_into_inventory(
                world,
                ctx,
                *pile_id,
                inventory_id,
                *quantity,
                unit.owner_id,
                unit.team_id,
                unit.affiliation,
            ) {
                Ok(report) if report.transfer.moved > 0 => {
                    ui.feedback_message = format!("Picked up {}.", report.transfer.moved);
                    InventoryIntentStatus::Applied
                }
                Ok(_) => {
                    ui.feedback_message = InventoryUiError::NoRoom.message();
                    InventoryIntentStatus::Rejected
                }
                Err(error) => {
                    ui.feedback_message = InventoryUiError::from_pile(error).message();
                    InventoryIntentStatus::Rejected
                }
            }
        }
        InventoryIntent::LootAll {
            corpse_inventory_id,
            actor_unit_id,
            destination_inventory_id,
        } => {
            if !can_access_pair(
                world,
                building_catalog,
                *actor_unit_id,
                *corpse_inventory_id,
                *destination_inventory_id,
                ui,
            ) {
                ui.feedback_message = InventoryUiError::AccessDenied.message();
                return InventoryIntentStatus::Rejected;
            }
            let mut any = false;
            while world
                .inventory_store()
                .get(*corpse_inventory_id)
                .is_some_and(|inv| !inv.placed_entries().is_empty())
            {
                let (inventory_store, instance_store) = world.inventory_runtime_mut();
                match loot_corpse_entry(
                    inventory_store,
                    instance_store,
                    ctx,
                    *corpse_inventory_id,
                    0,
                    *destination_inventory_id,
                    None,
                    TransferPlacementPolicy::MergeThenFirstFit,
                ) {
                    Ok(report) if report.moved > 0 => any = true,
                    Ok(_) => break,
                    Err(error) => {
                        ui.feedback_message = InventoryUiError::from_transfer(error).message();
                        return if any {
                            InventoryIntentStatus::Applied
                        } else {
                            InventoryIntentStatus::Rejected
                        };
                    }
                }
            }
            ui.feedback_message = if any {
                "Looted all.".into()
            } else {
                "Nothing to loot.".into()
            };
            if any {
                InventoryIntentStatus::Applied
            } else {
                InventoryIntentStatus::Ignored
            }
        }
        InventoryIntent::DepositGold {
            treasury_id,
            actor_unit_id,
            amount,
        } => {
            let Some(inventory_id) = world.get_unit(*actor_unit_id).and_then(|u| u.inventory_id)
            else {
                ui.feedback_message = InventoryUiError::AccessDenied.message();
                return InventoryIntentStatus::Rejected;
            };
            let quantity = match resolve_deposit_quantity(world, inventory_id, *amount) {
                Some(qty) if qty > 0 => qty,
                _ => {
                    ui.feedback_message = InventoryUiError::QuantityUnavailable.message();
                    return InventoryIntentStatus::Rejected;
                }
            };
            match deposit_gold(
                world,
                building_catalog,
                interaction_catalog,
                ctx,
                *actor_unit_id,
                inventory_id,
                *treasury_id,
                quantity,
                TreasuryAccessPolicy::default(),
                tick,
            ) {
                Ok(report) => {
                    ui.feedback_message = format!(
                        "Deposited {} gold. Treasury: {}.",
                        report.deposited_gold, report.treasury_balance_after
                    );
                    InventoryIntentStatus::Applied
                }
                Err(error) => {
                    ui.feedback_message = InventoryUiError::from_treasury(error).message();
                    InventoryIntentStatus::Rejected
                }
            }
        }
    }
}

fn resolve_deposit_quantity(
    world: &WorldData,
    inventory_id: InventoryId,
    amount: DepositGoldAmount,
) -> Option<u32> {
    let available = world
        .inventory_store()
        .get(inventory_id)
        .map(count_physical_gold)?;
    match amount {
        DepositGoldAmount::One => Some(1),
        DepositGoldAmount::Half => Some(half_stack_quantity(available)),
        DepositGoldAmount::All => Some(available),
    }
}

enum TransferMode {
    Full,
    One,
    Half,
}

fn transfer_with_policy(
    ui: &mut InventoryUiState,
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    building_catalog: &BuildingCatalog,
    source_inventory_id: InventoryId,
    source_entry_index: EntryIndex,
    destination_inventory_id: InventoryId,
    entry_revision: u64,
    mode: TransferMode,
) -> InventoryIntentStatus {
    if !revision_matches(
        world,
        source_inventory_id,
        source_entry_index,
        entry_revision,
    ) {
        ui.feedback_message = InventoryUiError::ItemChanged.message();
        return InventoryIntentStatus::Rejected;
    }
    if let Some(actor) = ui.actor_unit_id {
        if !can_access_pair(
            world,
            building_catalog,
            actor,
            source_inventory_id,
            destination_inventory_id,
            ui,
        ) {
            ui.feedback_message = InventoryUiError::AccessDenied.message();
            return InventoryIntentStatus::Rejected;
        }
    }
    let policy = TransferPlacementPolicy::MergeThenFirstFit;
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let result = match mode {
        TransferMode::Full => transfer_entry_full(
            inventory_store,
            instance_store,
            ctx,
            source_inventory_id,
            source_entry_index,
            destination_inventory_id,
            policy,
        ),
        TransferMode::One => transfer_one(
            inventory_store,
            instance_store,
            ctx,
            source_inventory_id,
            source_entry_index,
            destination_inventory_id,
            policy,
        ),
        TransferMode::Half => transfer_half(
            inventory_store,
            instance_store,
            ctx,
            source_inventory_id,
            source_entry_index,
            destination_inventory_id,
            policy,
        ),
    };
    match result {
        Ok(_) => {
            ui.feedback_message.clear();
            ui.invalidate_drag();
            InventoryIntentStatus::Applied
        }
        Err(error) => {
            ui.feedback_message = InventoryUiError::from_transfer(error).message();
            InventoryIntentStatus::Rejected
        }
    }
}

fn revision_matches(
    world: &WorldData,
    inventory_id: InventoryId,
    entry_index: EntryIndex,
    revision: u64,
) -> bool {
    entry_revision_for_inventory(world, inventory_id, entry_index) == revision
}

fn can_access_inventory(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    unit_id: UnitId,
    inventory_id: InventoryId,
    ui: &InventoryUiState,
) -> bool {
    if ui
        .right_inventory_id
        .is_some_and(|right| right == inventory_id)
        && ui.corpse_id.is_some()
    {
        return world.inventory_store().get(inventory_id).is_some();
    }
    matches!(
        can_unit_access_inventory(world, building_catalog, unit_id, inventory_id),
        InventoryAccessResult::Allowed
    )
}

fn can_access_pair(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    unit_id: UnitId,
    source: InventoryId,
    destination: InventoryId,
    ui: &InventoryUiState,
) -> bool {
    can_access_inventory(world, building_catalog, unit_id, source, ui)
        && can_access_inventory(world, building_catalog, unit_id, destination, ui)
}

fn find_corpse_for_inventory(world: &WorldData, inventory_id: InventoryId) -> Option<CorpseId> {
    for corpse_id in world.corpse_store().sorted_corpse_ids() {
        if world
            .corpse_store()
            .get(corpse_id)
            .and_then(|c| c.inventory_id)
            == Some(inventory_id)
        {
            return Some(corpse_id);
        }
    }
    None
}

fn apply_open_mode(ui: &mut InventoryUiState, world: &WorldData, mode: &InventoryOpenMode) {
    let mut state = InventoryUiState::default();
    state.open_mode(mode.clone());
    match mode {
        InventoryOpenMode::UnitOnly { unit_id } => {
            state.left_inventory_id = world.get_unit(*unit_id).and_then(|u| u.inventory_id);
        }
        InventoryOpenMode::DualTransfer {
            actor_unit_id,
            secondary_inventory_id,
            secondary_label,
        } => {
            state.left_inventory_id = world.get_unit(*actor_unit_id).and_then(|u| u.inventory_id);
            state.right_inventory_id = Some(*secondary_inventory_id);
            if secondary_label == "Corpse" {
                state.corpse_id = find_corpse_for_inventory(world, *secondary_inventory_id);
            }
        }
        InventoryOpenMode::WorldPile { actor_unit_id, .. } => {
            state.left_inventory_id = world.get_unit(*actor_unit_id).and_then(|u| u.inventory_id);
        }
        InventoryOpenMode::TreasuryDeposit { actor_unit_id, .. } => {
            state.left_inventory_id = world.get_unit(*actor_unit_id).and_then(|u| u.inventory_id);
        }
    }
    *ui = state;
}

fn reconcile_open_inventories(ui: &mut InventoryUiState, world: &WorldData) {
    if !ui.open {
        return;
    }
    let inventories_missing = [ui.left_inventory_id, ui.right_inventory_id]
        .into_iter()
        .flatten()
        .any(|id| world.inventory_store().get(id).is_none());
    if inventories_missing {
        ui.feedback_message = InventoryUiError::InventoryClosed.message();
        ui.close();
        return;
    }
    if let Some(corpse_id) = ui.corpse_id {
        if world.corpse_store().get(corpse_id).is_none() {
            ui.feedback_message = InventoryUiError::CorpseGone.message();
            ui.close();
        }
    }
    if let Some(pile_id) = ui.pile_id {
        if world.item_pile_store().get(pile_id).is_none() {
            ui.feedback_message = InventoryUiError::PileGone.message();
            ui.close();
        }
    }
    if let Some(treasury_id) = ui.treasury_id {
        if world.settlement_store().get_treasury(treasury_id).is_none() {
            ui.feedback_message = InventoryUiError::TreasuryUnavailable.message();
            ui.close();
        }
    }
    if let Some(actor) = ui.actor_unit_id {
        if world.get_unit(actor).is_none() {
            ui.close();
        }
    }
}

/// Queue an inventory open from an interact command at a world target.
pub fn try_queue_inventory_open_from_interact(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    doodad_catalog: &crate::world::DoodadCatalog,
    footprint_catalog: &crate::world::FootprintCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    unit_catalog: &crate::world::UnitCatalog,
    weapon_catalog: &crate::world::WeaponCatalog,
    actor_unit_id: UnitId,
    target: crate::client::commands::CommandTarget,
    queue: &mut InventoryIntentQueue,
) -> bool {
    use crate::world::{
        InteractionQueryContext, InteractionTargetRef, InteractionType, query_world_interaction,
    };

    let position = match target {
        crate::client::commands::CommandTarget::Terrain { position } => position,
        crate::client::commands::CommandTarget::Unit { unit_id } => world
            .get_unit(unit_id)
            .map(|u| u.placement.position)
            .unwrap_or_else(|| {
                crate::world::WorldPosition::new(
                    crate::world::ChunkCoord::new(0, 0),
                    crate::world::LocalPosition::new(Vec3::ZERO),
                )
            }),
    };
    let ctx = InteractionQueryContext::new(
        world,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        interaction_catalog,
        unit_catalog,
        weapon_catalog,
    );
    let Some(interaction) = query_world_interaction(&ctx, position) else {
        return false;
    };
    match interaction.interaction_type {
        InteractionType::Container => {
            if let InteractionTargetRef::Building(building_id) = interaction.target {
                if let Ok(mode) = try_open_container_inventory(
                    world,
                    building_catalog,
                    actor_unit_id,
                    building_id,
                ) {
                    queue.push(InventoryIntent::Open(mode));
                    return true;
                }
            }
        }
        InteractionType::Treasury => {
            if let InteractionTargetRef::Building(building_id) = interaction.target {
                if let Ok(mode) = try_open_treasury_deposit(
                    world,
                    building_catalog,
                    interaction_catalog,
                    actor_unit_id,
                    building_id,
                ) {
                    queue.push(InventoryIntent::Open(mode));
                    return true;
                }
            }
        }
        InteractionType::Corpse => {
            if let InteractionTargetRef::Corpse(corpse_id) = interaction.target {
                if let Ok(mode) = try_open_corpse_inventory(world, actor_unit_id, corpse_id) {
                    queue.push(InventoryIntent::Open(mode));
                    return true;
                }
            }
        }
        InteractionType::ItemPile => {
            if let InteractionTargetRef::ItemPile(pile_id) = interaction.target {
                queue.push(InventoryIntent::Open(try_open_pile_inventory(
                    actor_unit_id,
                    pile_id,
                )));
                return true;
            }
        }
        _ => {}
    }
    false
}

/// Resolve settlement treasury deposit UI for a unit.
pub fn try_open_treasury_deposit(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    actor_unit_id: UnitId,
    building_id: BuildingId,
) -> Result<InventoryOpenMode, InventoryUiError> {
    use crate::world::{TreasuryAccessResult, can_unit_deposit_to_treasury};

    let Some(settlement_id) = world
        .settlement_store()
        .settlement_for_building(building_id)
    else {
        return Err(InventoryUiError::TreasuryUnavailable);
    };
    let Some(treasury_id) = world
        .settlement_store()
        .treasury_for_settlement(settlement_id)
    else {
        return Err(InventoryUiError::TreasuryUnavailable);
    };
    let access = can_unit_deposit_to_treasury(
        world,
        building_catalog,
        interaction_catalog,
        world.settlement_store(),
        actor_unit_id,
        treasury_id,
        TreasuryAccessPolicy::default(),
    );
    if !matches!(access, TreasuryAccessResult::Allowed) {
        return Err(InventoryUiError::from_treasury_access(access));
    }
    let label = world
        .settlement_store()
        .get_settlement(settlement_id)
        .map(|s| s.display_name.clone())
        .unwrap_or_else(|| "Treasury".into());
    Ok(InventoryOpenMode::TreasuryDeposit {
        actor_unit_id,
        treasury_id,
        settlement_id,
        building_id,
        label,
    })
}

/// Resolve building container open for a unit.
pub fn try_open_container_inventory(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    actor_unit_id: UnitId,
    building_id: BuildingId,
) -> Result<InventoryOpenMode, InventoryUiError> {
    let access =
        can_unit_access_building_inventory(world, building_catalog, actor_unit_id, building_id);
    if !access.is_allowed() {
        return Err(InventoryUiError::AccessDenied);
    }
    let building = world
        .get_building(building_id)
        .ok_or(InventoryUiError::InventoryClosed)?;
    let inventory_id = building
        .inventory_id
        .ok_or(InventoryUiError::InventoryClosed)?;
    let label = building_catalog
        .get(&building.definition_id)
        .map(|d| d.display_name.clone())
        .unwrap_or_else(|| "Container".into());
    Ok(InventoryOpenMode::DualTransfer {
        actor_unit_id,
        secondary_inventory_id: inventory_id,
        secondary_label: label,
    })
}

/// Resolve corpse loot open for a unit.
pub fn try_open_corpse_inventory(
    world: &WorldData,
    actor_unit_id: UnitId,
    corpse_id: CorpseId,
) -> Result<InventoryOpenMode, InventoryUiError> {
    let corpse = world
        .corpse_store()
        .get(corpse_id)
        .ok_or(InventoryUiError::CorpseGone)?;
    let inventory_id = corpse
        .inventory_id
        .ok_or(InventoryUiError::InventoryClosed)?;
    Ok(InventoryOpenMode::DualTransfer {
        actor_unit_id,
        secondary_inventory_id: inventory_id,
        secondary_label: "Corpse".into(),
    })
}

/// Resolve world pile interaction.
pub fn try_open_pile_inventory(actor_unit_id: UnitId, pile_id: ItemPileId) -> InventoryOpenMode {
    InventoryOpenMode::WorldPile {
        actor_unit_id,
        pile_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, BuildingSource, ChunkCoord, ChunkData, ChunkLayout, Heightfield,
        LocalPosition, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource,
        create_building_with_inventory, create_unit_with_inventory, starter_building_definitions,
        starter_inventory_profile_definitions, starter_item_category_definitions,
        starter_item_definitions, starter_unit_definitions,
    };
    use bevy::prelude::Quat;

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

    fn test_ctx<'a>(
        items: &'a ItemCatalog,
        categories: &'a ItemCategoryCatalog,
        profiles: &'a InventoryProfileCatalog,
    ) -> InventoryCatalogCtx<'a> {
        InventoryCatalogCtx::new(items, categories, profiles)
    }

    #[test]
    fn container_open_requires_access() {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let building_categories = crate::world::BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &building_categories)
                .unwrap();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let ctx = test_ctx(&items, &categories, &profiles);
        let mut world = test_world();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            crate::world::WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            &ctx,
        )
        .unwrap();
        let chest = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &crate::world::BuildingDefinitionId::new("storage_chest"),
            crate::world::WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(5.0, 0.0, 5.0)),
            ),
            Quat::IDENTITY,
            BuildingSource::Authored,
            crate::world::BuildingOwnership {
                owner_id: Some(crate::world::OwnerId::new(99)),
                team_id: None,
                affiliation: crate::world::Affiliation::Hostile,
            },
            None,
            &ctx,
        )
        .unwrap();
        world.mutate_building(chest.id, |b| {
            b.lifecycle_state = crate::world::BuildingLifecycleState::Complete;
        });
        let mode = try_open_container_inventory(&world, &building_catalog, unit.id, chest.id);
        assert!(mode.is_err());
    }

    #[test]
    fn open_unit_only_binds_left_inventory() {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let ctx = test_ctx(&items, &categories, &profiles);
        let mut world = test_world();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            crate::world::WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            &ctx,
        )
        .unwrap();
        let expected = unit.inventory_id;
        let mut ui = InventoryUiState::default();
        let status = dispatch_one(
            &InventoryIntent::Open(InventoryOpenMode::UnitOnly { unit_id: unit.id }),
            &mut ui,
            &mut world,
            &ctx,
            &BuildingCatalog::default(),
            &BuildingInteractionProfileCatalog::default(),
            &ItemPileSettings::default(),
            0,
        );
        assert_eq!(status, InventoryIntentStatus::Applied);
        assert!(ui.open);
        assert_eq!(ui.left_inventory_id, expected);
        assert_eq!(ui.actor_unit_id, Some(unit.id));
    }

    #[test]
    fn stale_revision_rejects_move() {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let ctx = test_ctx(&items, &categories, &profiles);
        let mut world = test_world();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            crate::world::WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            &ctx,
        )
        .unwrap();
        let inventory_id = unit.inventory_id.unwrap();
        let mut ui = InventoryUiState::default();
        ui.open_mode(InventoryOpenMode::UnitOnly { unit_id: unit.id });
        ui.left_inventory_id = Some(inventory_id);
        let status = dispatch_one(
            &InventoryIntent::MoveEntry {
                inventory_id,
                entry_index: 99,
                anchor_x: 0,
                anchor_y: 0,
                entry_revision: 1,
            },
            &mut ui,
            &mut world,
            &ctx,
            &BuildingCatalog::default(),
            &BuildingInteractionProfileCatalog::default(),
            &ItemPileSettings::default(),
            0,
        );
        assert_eq!(status, InventoryIntentStatus::Rejected);
        assert_eq!(ui.feedback_message, InventoryUiError::ItemChanged.message());
    }

    #[test]
    fn treasury_deposit_open_sets_separate_gold_fields() {
        use crate::world::{
            BuildingOwnership, BuildingSource, SettlementOwnership, create_building,
            create_settlement_with_treasury, physical_gold_item_id, place_stack_first_fit,
        };
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let building_categories = crate::world::BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &building_categories)
                .unwrap();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let ctx = test_ctx(&items, &categories, &profiles);
        let mut world = test_world();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            crate::world::WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            &ctx,
        )
        .unwrap();
        let inventory_id = unit.inventory_id.unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            physical_gold_item_id(),
            12,
        )
        .unwrap();
        let building = create_building(
            &building_catalog,
            &mut world,
            &crate::world::BuildingDefinitionId::new("settlement_core"),
            crate::world::WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(1.5, 0.0, 1.5)),
            ),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
        )
        .unwrap();
        world.mutate_building(building.id, |b| {
            b.lifecycle_state = crate::world::BuildingLifecycleState::Complete;
        });
        let report = create_settlement_with_treasury(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            building.id,
            "Town",
            SettlementOwnership::player_default(),
            building.placement.position,
            0,
        )
        .unwrap();
        let mode = try_open_treasury_deposit(
            &world,
            &building_catalog,
            &interaction_catalog,
            unit.id,
            building.id,
        )
        .unwrap();
        let mut ui = InventoryUiState::default();
        dispatch_one(
            &InventoryIntent::Open(mode),
            &mut ui,
            &mut world,
            &ctx,
            &building_catalog,
            &interaction_catalog,
            &ItemPileSettings::default(),
            0,
        );
        assert!(ui.treasury_deposit_open());
        assert_eq!(ui.treasury_id, Some(report.treasury_id));
        assert_ne!(ui.left_inventory_id, ui.right_inventory_id);
        assert!(ui.right_inventory_id.is_none());
    }
}
