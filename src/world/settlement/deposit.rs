//! Authoritative treasury deposit API (ADR-093 I7).

use bevy::prelude::*;

use super::access::{TreasuryAccessPolicy, can_unit_deposit_to_treasury};
use super::error::TreasuryError;
use super::id::TreasuryId;
use super::record::TreasuryTransactionRecord;
use crate::world::building::{BuildingCatalog, BuildingInteractionProfileCatalog};
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryId, InventoryOwnerRef, consume_stack_item, count_physical_gold,
    physical_gold_item_id,
};
use crate::world::{UnitId, WorldData};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositGoldReport {
    pub treasury_id: TreasuryId,
    pub deposited_gold: u32,
    pub treasury_balance_after: u64,
    pub source_inventory_remaining_gold: u32,
}

/// Deposit physical gold from an inventory into abstract settlement wealth.
///
/// Atomic: physical removal and treasury credit succeed together or roll back.
pub fn deposit_gold(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    ctx: &InventoryCatalogCtx<'_>,
    depositor_unit_id: UnitId,
    source_inventory_id: InventoryId,
    treasury_id: TreasuryId,
    quantity: u32,
    policy: TreasuryAccessPolicy,
    tick: u64,
) -> Result<DepositGoldReport, TreasuryError> {
    if quantity == 0 {
        return Err(TreasuryError::InvalidQuantity { requested: 0 });
    }

    let access = can_unit_deposit_to_treasury(
        world,
        building_catalog,
        interaction_catalog,
        world.settlement_store(),
        depositor_unit_id,
        treasury_id,
        policy,
    );
    match access {
        super::access::TreasuryAccessResult::Allowed => {}
        super::access::TreasuryAccessResult::Denied(error) => return Err(error),
    }

    let Some(unit) = world.get_unit(depositor_unit_id) else {
        return Err(TreasuryError::RequesterMissing(depositor_unit_id));
    };
    let Some(unit_inventory_id) = unit.inventory_id else {
        return Err(TreasuryError::SourceInventoryNotFound(source_inventory_id));
    };
    if unit_inventory_id != source_inventory_id {
        return Err(TreasuryError::SourceInventoryNotOwnedByUnit {
            inventory_id: source_inventory_id,
            unit_id: depositor_unit_id,
        });
    }

    let inventory = world
        .inventory_store()
        .get(source_inventory_id)
        .ok_or(TreasuryError::SourceInventoryNotFound(source_inventory_id))?;
    match inventory.owner() {
        InventoryOwnerRef::Unit(owner) if *owner == depositor_unit_id => {}
        _ => {
            return Err(TreasuryError::SourceInventoryNotOwnedByUnit {
                inventory_id: source_inventory_id,
                unit_id: depositor_unit_id,
            });
        }
    }

    let available = count_physical_gold(inventory);
    if available < quantity {
        return Err(TreasuryError::InsufficientPhysicalGold {
            available,
            requested: quantity,
        });
    }

    let (settlement_id, balance_before) = {
        let treasury = world
            .settlement_store()
            .get_treasury(treasury_id)
            .ok_or(TreasuryError::TreasuryNotFound(treasury_id))?;
        (treasury.settlement_id, treasury.balance_gold)
    };
    if balance_before.checked_add(u64::from(quantity)).is_none() {
        return Err(TreasuryError::QuantityOverflow);
    }

    let inventory_backup = inventory.clone();

    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let consumed = match consume_stack_item(
        inventory_store,
        instance_store,
        ctx,
        source_inventory_id,
        &physical_gold_item_id(),
        quantity,
    ) {
        Ok(amount) => amount,
        Err(error) => return Err(TreasuryError::Inventory(error)),
    };
    if consumed != quantity {
        if let Some(record) = inventory_store.get_mut(source_inventory_id) {
            *record = inventory_backup;
        }
        return Err(TreasuryError::InsufficientPhysicalGold {
            available: consumed,
            requested: quantity,
        });
    }

    let treasury = world
        .settlement_store_mut()
        .get_treasury_mut(treasury_id)
        .ok_or(TreasuryError::TreasuryNotFound(treasury_id))?;
    let new_balance = treasury
        .balance_gold
        .checked_add(u64::from(consumed))
        .ok_or(TreasuryError::QuantityOverflow)?;
    treasury.balance_gold = new_balance;

    let remaining = world
        .inventory_store()
        .get(source_inventory_id)
        .map(count_physical_gold)
        .unwrap_or(0);

    world
        .settlement_store_mut()
        .push_transaction(TreasuryTransactionRecord {
            tick,
            treasury_id,
            settlement_id,
            unit_id: Some(depositor_unit_id),
            source_inventory_id: Some(source_inventory_id),
            deposited_gold: consumed,
            balance_after: new_balance,
        });

    Ok(DepositGoldReport {
        treasury_id,
        deposited_gold: consumed,
        treasury_balance_after: new_balance,
        source_inventory_remaining_gold: remaining,
    })
}
