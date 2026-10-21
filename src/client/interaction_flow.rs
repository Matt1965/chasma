//! Authoritative client routing for unit social interactions (trade vs peer transfer).

use bevy::prelude::*;

use crate::client::inventory_dispatch::apply_inventory_open_mode;
use crate::client::inventory_intent::{InventoryIntent, InventoryIntentQueue, InventoryOpenMode};
use crate::client::TradeSessionState;
use crate::ui::gameplay::InventoryUiState;
use crate::ui::gameplay::{FloatingGameplayWindowId, FloatingGameplayWindowRegistry};
use crate::ui::gameplay::is_player_to_player_trade_pair;
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
    if is_player_to_player_trade_pair(world, buyer_unit_id, seller_unit_id) {
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

pub fn peer_transfer_still_valid(
    world: &WorldData,
    actor_unit_id: UnitId,
    peer_unit_id: UnitId,
) -> bool {
    if !is_player_to_player_trade_pair(world, actor_unit_id, peer_unit_id) {
        return false;
    }
    if !units_within_dialogue_range(world, actor_unit_id, peer_unit_id) {
        return false;
    }
    let actor = world.get_unit(actor_unit_id);
    let peer = world.get_unit(peer_unit_id);
    if !actor.is_some_and(crate::world::is_unit_alive)
        || !peer.is_some_and(crate::world::is_unit_alive)
    {
        return false;
    }
    match (actor, peer) {
        (Some(a), Some(t)) => a.current_space_id == t.current_space_id,
        _ => false,
    }
}

pub fn reconcile_peer_transfer_session(inventory_ui: &mut InventoryUiState, world: &WorldData) {
    if !inventory_ui.open {
        return;
    }
    let (actor, peer) = match (inventory_ui.actor_unit_id, inventory_ui.peer_unit_id) {
        (Some(a), Some(p)) => (a, p),
        _ => return,
    };
    if peer_transfer_still_valid(world, actor, peer) {
        return;
    }
    inventory_ui.feedback_message = "Transfer ended.".into();
    inventory_ui.close();
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
    if is_player_to_player_trade_pair(world, buyer_unit_id, seller_unit_id) {
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
    let mode = InventoryOpenMode::UnitOnly {
        unit_id: buyer_unit_id,
    };
    queue.push(InventoryIntent::Open(mode.clone()));
    apply_inventory_open_mode(inventory_ui, world, &mode);
    true
}

pub fn open_peer_unit_transfer(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    trade: &mut TradeSessionState,
    inventory_ui: &mut InventoryUiState,
    queue: &mut InventoryIntentQueue,
    registry: &mut FloatingGameplayWindowRegistry,
    actor_unit_id: UnitId,
    peer_unit_id: UnitId,
) -> bool {
    if actor_unit_id == peer_unit_id {
        return false;
    }
    if !is_player_to_player_trade_pair(world, actor_unit_id, peer_unit_id) {
        return false;
    }
    if trade.open {
        trade.close();
        inventory_ui.trade_seller_inventory_id = None;
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
    registry.focus_window(FloatingGameplayWindowId::PeerUnitInventory);
    let mode = InventoryOpenMode::DualTransfer {
        actor_unit_id,
        peer_unit_id: Some(peer_unit_id),
        secondary_inventory_id: peer_inventory_id,
        secondary_label: peer_label,
    };
    queue.push(InventoryIntent::Open(mode.clone()));
    apply_inventory_open_mode(inventory_ui, world, &mode);
    true
}

pub fn unit_display_name(world: &WorldData, unit_catalog: &UnitCatalog, unit_id: UnitId) -> String {
    world
        .get_unit(unit_id)
        .and_then(|record| unit_catalog.get(&record.definition_id))
        .map(|def| def.display_name.clone())
        .unwrap_or_else(|| format!("Unit {}", unit_id.raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::inventory_intent::InventoryIntentQueue;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, InventoryCatalogCtx,
        InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, LocalPosition, UnitCatalog,
        UnitDefinitionId, UnitOwnership, UnitSource, WorldData, WorldPosition,
        create_unit_with_inventory, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
    };
    use bevy::prelude::Vec3;

    #[test]
    fn open_peer_unit_transfer_opens_dual_inventory_ui() {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let actor = create_unit_with_inventory(
            &unit_catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
            &mut world,
            &UnitDefinitionId::new("bandit"),
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(0.0, 0.0, 0.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::player_default(),
            &ctx,
        )
        .unwrap()
        .id;
        let peer = create_unit_with_inventory(
            &unit_catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
            &mut world,
            &UnitDefinitionId::new("bandit"),
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(1.0, 0.0, 0.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::player_default(),
            &ctx,
        )
        .unwrap()
        .id;
        let mut trade = TradeSessionState::default();
        let mut inventory_ui = InventoryUiState::default();
        let mut queue = InventoryIntentQueue::default();
        let mut registry = FloatingGameplayWindowRegistry::default();

        assert!(open_peer_unit_transfer(
            &world,
            &unit_catalog,
            &mut trade,
            &mut inventory_ui,
            &mut queue,
            &mut registry,
            actor,
            peer,
        ));
        assert!(inventory_ui.open);
        assert_eq!(inventory_ui.actor_unit_id, Some(actor));
        assert_eq!(inventory_ui.peer_unit_id, Some(peer));
        assert!(inventory_ui.left_inventory_id.is_some());
        assert!(inventory_ui.right_inventory_id.is_some());
    }
}
