//! Authoritative client routing for unit social interactions (trade vs peer transfer).

use bevy::prelude::*;

use crate::client::inventory_intent::{InventoryIntent, InventoryIntentQueue, InventoryOpenMode};
use crate::client::TradeSessionState;
use crate::ui::gameplay::InventoryUiState;
use crate::ui::gameplay::{FloatingGameplayWindowId, FloatingGameplayWindowRegistry};
use crate::ui::gameplay::uses_player_trade_only_menu;
use crate::world::relationship::AuthoredRelationshipCatalog;
use crate::world::{
    DialogueActionKind, UnitCatalog, UnitId, WorldData, evaluate_dialogue_option,
    unit_supports_dialogue, units_within_dialogue_range,
};

/// Where NPC seller stock is resolved from today (shop aggregation will extend this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellerStockSource {
    UnitPersonalInventory {
        inventory_id: crate::world::InventoryId,
    },
}

pub fn npc_trade_still_valid(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    buyer_unit_id: UnitId,
    seller_unit_id: UnitId,
) -> bool {
    if !units_within_dialogue_range(world, buyer_unit_id, seller_unit_id) {
        return false;
    }
    let buyer = world.get_unit(buyer_unit_id);
    let seller = world.get_unit(seller_unit_id);
    if !buyer.is_some_and(crate::world::is_unit_alive)
        || !seller.is_some_and(crate::world::is_unit_alive)
    {
        return false;
    }
    if uses_player_trade_only_menu(world, buyer_unit_id, seller_unit_id) {
        return false;
    }
    if !seller.is_some_and(unit_supports_dialogue) {
        return false;
    }
    let standing = world.relationship_standing_store();
    evaluate_dialogue_option(
        world,
        authored_relationships,
        standing,
        buyer_unit_id,
        seller_unit_id,
        DialogueActionKind::Trade,
    )
    .is_available()
}

pub fn open_npc_trade_session(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    trade: &mut TradeSessionState,
    inventory_ui: &mut InventoryUiState,
    queue: &mut InventoryIntentQueue,
    registry: &mut FloatingGameplayWindowRegistry,
    buyer_unit_id: UnitId,
    seller_unit_id: UnitId,
) -> bool {
    if uses_player_trade_only_menu(world, buyer_unit_id, seller_unit_id) {
        return false;
    }
    if !npc_trade_still_valid(world, authored_relationships, buyer_unit_id, seller_unit_id) {
        inventory_ui.feedback_message = "Cannot trade with them right now.".into();
        return false;
    }
    let seller = world.get_unit(seller_unit_id);
    let stock_source = seller
        .and_then(|record| record.inventory_id)
        .map(|inventory_id| SellerStockSource::UnitPersonalInventory { inventory_id });

    trade.open = true;
    trade.buyer_unit_id = Some(buyer_unit_id);
    trade.seller_unit_id = Some(seller_unit_id);
    trade.seller_stock_source = stock_source;
    trade.seller_inventory_id = stock_source.map(|source| match source {
        SellerStockSource::UnitPersonalInventory { inventory_id } => inventory_id,
    });
    trade.content_revision += 1;

    inventory_ui.trade_seller_inventory_id = trade.seller_inventory_id;
    inventory_ui.actor_unit_id = Some(buyer_unit_id);
    inventory_ui.feedback_message = if trade.seller_inventory_id.is_none() {
        "This merchant has no stock container.".into()
    } else {
        String::new()
    };

    registry.focus_window(FloatingGameplayWindowId::Trade);
    registry.focus_window(FloatingGameplayWindowId::UnitInventory);
    queue.push(InventoryIntent::Open(InventoryOpenMode::UnitOnly { unit_id: buyer_unit_id }));
    true
}

pub fn open_peer_unit_transfer(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    inventory_ui: &mut InventoryUiState,
    queue: &mut InventoryIntentQueue,
    registry: &mut FloatingGameplayWindowRegistry,
    actor_unit_id: UnitId,
    peer_unit_id: UnitId,
) -> bool {
    if actor_unit_id == peer_unit_id {
        return false;
    }
    if !uses_player_trade_only_menu(world, actor_unit_id, peer_unit_id) {
        return false;
    }
    if !units_within_dialogue_range(world, actor_unit_id, peer_unit_id) {
        inventory_ui.feedback_message = "Too far away.".into();
        return false;
    }
    let peer_inventory = world
        .get_unit(peer_unit_id)
        .and_then(|unit| unit.inventory_id);
    let Some(peer_inventory_id) = peer_inventory else {
        inventory_ui.feedback_message = "They are not carrying inventory.".into();
        return false;
    };
    let peer_label = unit_display_name(world, unit_catalog, peer_unit_id);
    registry.focus_window(FloatingGameplayWindowId::UnitInventory);
    queue.push(InventoryIntent::Open(InventoryOpenMode::DualTransfer {
        actor_unit_id,
        peer_unit_id: Some(peer_unit_id),
        secondary_inventory_id: peer_inventory_id,
        secondary_label: peer_label,
    }));
    true
}

pub fn unit_display_name(world: &WorldData, unit_catalog: &UnitCatalog, unit_id: UnitId) -> String {
    world
        .get_unit(unit_id)
        .and_then(|record| unit_catalog.get(&record.definition_id))
        .map(|def| def.display_name.clone())
        .unwrap_or_else(|| format!("Unit {}", unit_id.raw()))
}
