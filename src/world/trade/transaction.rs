//! Authoritative vendor buy/sell transfers (inventory + physical gold).

use super::pricing::{trade_line_total, vendor_buy_price_gold, vendor_sell_price_gold};
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryError, TransferError, TransferPlacementPolicy, TransferReport,
    consume_stack_item, count_physical_gold, physical_gold_item_id, place_stack_first_fit,
    transfer_entry_full, transfer_half, transfer_one, transfer_stack_quantity,
};
use crate::world::{
    ItemCatalog, ItemDefinitionId, UnitId, WorldData, half_stack_quantity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeDirection {
    BuyFromVendor,
    SellToVendor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeTransactionError {
    NotATradePair,
    ItemNotPriced,
    InsufficientBuyerGold,
    GoldCreditFailed,
    Inventory(InventoryError),
    Transfer(TransferError),
    BuyerInventoryMissing,
    SellerInventoryMissing,
}

impl From<InventoryError> for TradeTransactionError {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}

impl From<TransferError> for TradeTransactionError {
    fn from(value: TransferError) -> Self {
        Self::Transfer(value)
    }
}

pub fn classify_trade_transfer(
    buyer_inventory_id: crate::world::InventoryId,
    seller_inventory_id: crate::world::InventoryId,
    source_inventory_id: crate::world::InventoryId,
    destination_inventory_id: crate::world::InventoryId,
) -> Option<TradeDirection> {
    if source_inventory_id == seller_inventory_id && destination_inventory_id == buyer_inventory_id {
        return Some(TradeDirection::BuyFromVendor);
    }
    if source_inventory_id == buyer_inventory_id && destination_inventory_id == seller_inventory_id {
        return Some(TradeDirection::SellToVendor);
    }
    None
}

fn unit_price_gold(
    items: &ItemCatalog,
    item_definition_id: &ItemDefinitionId,
    direction: TradeDirection,
) -> Result<u32, TradeTransactionError> {
    let Some(definition) = items.get(item_definition_id) else {
        return Err(TradeTransactionError::ItemNotPriced);
    };
    if definition.base_value_gold == 0 && item_definition_id != &physical_gold_item_id() {
        return Err(TradeTransactionError::ItemNotPriced);
    }
    Ok(match direction {
        TradeDirection::BuyFromVendor => vendor_sell_price_gold(definition.base_value_gold),
        TradeDirection::SellToVendor => vendor_buy_price_gold(definition.base_value_gold),
    })
}

fn quantity_for_transfer_mode(
    world: &WorldData,
    source_inventory_id: crate::world::InventoryId,
    source_entry_index: usize,
    mode: TradeTransferMode,
) -> Result<u32, TradeTransactionError> {
    let record = world
        .inventory_store()
        .get(source_inventory_id)
        .ok_or(TradeTransactionError::SellerInventoryMissing)?;
    let entry = record
        .placed_entries()
        .get(source_entry_index)
        .ok_or(TradeTransactionError::Transfer(TransferError::SourceEntryMissing {
            inventory_id: source_inventory_id,
            entry_index: source_entry_index,
        }))?;
    Ok(match &entry.contents {
        crate::world::InventoryEntryContents::Stack { quantity, .. } => match mode {
            TradeTransferMode::Full | TradeTransferMode::ToCell { .. } => *quantity,
            TradeTransferMode::One => 1,
            TradeTransferMode::Half => half_stack_quantity(*quantity),
        },
        crate::world::InventoryEntryContents::Unique { .. } => 1,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeTransferMode {
    Full,
    One,
    Half,
    ToCell {
        anchor_x: u8,
        anchor_y: u8,
    },
}

pub struct TradeTransferRequest {
    pub buyer_unit_id: UnitId,
    pub buyer_inventory_id: crate::world::InventoryId,
    pub seller_inventory_id: crate::world::InventoryId,
    pub source_inventory_id: crate::world::InventoryId,
    pub source_entry_index: usize,
    pub destination_inventory_id: crate::world::InventoryId,
    pub mode: TradeTransferMode,
}

pub struct TradeTransferOutcome {
    pub report: TransferReport,
    pub gold_delta_for_buyer: i64,
}

pub fn execute_trade_transfer(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    items: &ItemCatalog,
    request: &TradeTransferRequest,
) -> Result<TradeTransferOutcome, TradeTransactionError> {
    let direction = classify_trade_transfer(
        request.buyer_inventory_id,
        request.seller_inventory_id,
        request.source_inventory_id,
        request.destination_inventory_id,
    )
    .ok_or(TradeTransactionError::NotATradePair)?;

    let item_definition_id = item_definition_for_entry(
        world,
        request.source_inventory_id,
        request.source_entry_index,
    )?;
    let unit_price = unit_price_gold(items, &item_definition_id, direction)?;

    let planned_qty = quantity_for_transfer_mode(
        world,
        request.source_inventory_id,
        request.source_entry_index,
        match request.mode {
            TradeTransferMode::Full => TradeTransferMode::Full,
            TradeTransferMode::One => TradeTransferMode::One,
            TradeTransferMode::Half => TradeTransferMode::Half,
            TradeTransferMode::ToCell { .. } => TradeTransferMode::Full,
        },
    )?;

    if planned_qty == 0 {
        return Err(TradeTransactionError::Transfer(TransferError::SourceEntryMissing {
            inventory_id: request.source_inventory_id,
            entry_index: request.source_entry_index,
        }));
    }

    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let policy = match request.mode {
        TradeTransferMode::ToCell { anchor_x, anchor_y } => TransferPlacementPolicy::ExactCell {
            x: anchor_x,
            y: anchor_y,
        },
        _ => TransferPlacementPolicy::MergeThenFirstFit,
    };

    let report = match request.mode {
        TradeTransferMode::Full | TradeTransferMode::ToCell { .. } => transfer_entry_full(
            inventory_store,
            instance_store,
            ctx,
            request.source_inventory_id,
            request.source_entry_index,
            request.destination_inventory_id,
            policy,
        )?,
        TradeTransferMode::One => transfer_one(
            inventory_store,
            instance_store,
            ctx,
            request.source_inventory_id,
            request.source_entry_index,
            request.destination_inventory_id,
            policy,
        )?,
        TradeTransferMode::Half => transfer_half(
            inventory_store,
            instance_store,
            ctx,
            request.source_inventory_id,
            request.source_entry_index,
            request.destination_inventory_id,
            policy,
        )?,
    };

    if report.moved == 0 {
        return Ok(TradeTransferOutcome {
            report,
            gold_delta_for_buyer: 0,
        });
    }

    let line_cost = trade_line_total(unit_price, report.moved);

    match direction {
        TradeDirection::BuyFromVendor => {
            let buyer_gold = inventory_store
                .get(request.buyer_inventory_id)
                .map(count_physical_gold)
                .ok_or(TradeTransactionError::BuyerInventoryMissing)?;
            if buyer_gold < line_cost {
                let _ = transfer_stack_back(
                    inventory_store,
                    instance_store,
                    ctx,
                    request.destination_inventory_id,
                    request.source_inventory_id,
                    &item_definition_id,
                    report.moved,
                );
                return Err(TradeTransactionError::InsufficientBuyerGold);
            }
            let consumed = consume_stack_item(
                inventory_store,
                instance_store,
                ctx,
                request.buyer_inventory_id,
                &physical_gold_item_id(),
                line_cost,
            )?;
            if consumed != line_cost {
                // Roll back item transfer by moving back (best-effort consistency).
                let _ = transfer_stack_back(
                    inventory_store,
                    instance_store,
                    ctx,
                    request.destination_inventory_id,
                    request.source_inventory_id,
                    &item_definition_id,
                    report.moved,
                );
                return Err(TradeTransactionError::InsufficientBuyerGold);
            }
            Ok(TradeTransferOutcome {
                report,
                gold_delta_for_buyer: -(line_cost as i64),
            })
        }
        TradeDirection::SellToVendor => {
            if !credit_physical_gold(
                inventory_store,
                instance_store,
                ctx,
                request.buyer_inventory_id,
                line_cost,
            )? {
                let _ = transfer_stack_back(
                    inventory_store,
                    instance_store,
                    ctx,
                    request.destination_inventory_id,
                    request.source_inventory_id,
                    &item_definition_id,
                    report.moved,
                );
                return Err(TradeTransactionError::GoldCreditFailed);
            }
            Ok(TradeTransferOutcome {
                report,
                gold_delta_for_buyer: line_cost as i64,
            })
        }
    }
}

fn item_definition_for_entry(
    world: &WorldData,
    inventory_id: crate::world::InventoryId,
    entry_index: usize,
) -> Result<ItemDefinitionId, TradeTransactionError> {
    let record = world
        .inventory_store()
        .get(inventory_id)
        .ok_or(TradeTransactionError::SellerInventoryMissing)?;
    let entry = record
        .placed_entries()
        .get(entry_index)
        .ok_or(TradeTransactionError::Transfer(TransferError::SourceEntryMissing {
            inventory_id,
            entry_index,
        }))?;
    match &entry.contents {
        crate::world::InventoryEntryContents::Stack {
            item_definition_id, ..
        } => Ok(item_definition_id.clone()),
        crate::world::InventoryEntryContents::Unique { item_instance_id } => world
            .item_instance_store()
            .get(*item_instance_id)
            .map(|i| i.definition_id.clone())
            .ok_or(TradeTransactionError::ItemNotPriced),
    }
}

fn credit_physical_gold(
    inventory_store: &mut crate::world::inventory::InventoryStore,
    instance_store: &mut crate::world::inventory::ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    buyer_inventory_id: crate::world::InventoryId,
    amount: u32,
) -> Result<bool, TradeTransactionError> {
    if amount == 0 {
        return Ok(true);
    }
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        buyer_inventory_id,
        physical_gold_item_id(),
        amount,
    )
    .map(|_| true)
    .map_err(TradeTransactionError::from)
}

fn transfer_stack_back(
    inventory_store: &mut crate::world::inventory::InventoryStore,
    instance_store: &mut crate::world::inventory::ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    from_inventory: crate::world::InventoryId,
    to_inventory: crate::world::InventoryId,
    item_definition_id: &ItemDefinitionId,
    quantity: u32,
) -> Result<(), TransferError> {
    let from = inventory_store
        .get(from_inventory)
        .ok_or(TransferError::DestinationInventoryNotFound(from_inventory))?;
    let entry_index = from
        .placed_entries()
        .iter()
        .position(|entry| {
            matches!(
                &entry.contents,
                crate::world::InventoryEntryContents::Stack {
                    item_definition_id: id,
                    quantity: qty,
                } if id == item_definition_id && *qty >= quantity
            )
        })
        .ok_or(TransferError::SourceEntryMissing {
            inventory_id: from_inventory,
            entry_index: 0,
        })?;
    transfer_stack_quantity(
        inventory_store,
        instance_store,
        ctx,
        from_inventory,
        entry_index,
        to_inventory,
        quantity,
        TransferPlacementPolicy::MergeThenFirstFit,
        false,
    )?;
    Ok(())
}
