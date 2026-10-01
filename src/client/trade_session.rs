//! Active vendor trade session (buyer + seller binding for UI and transfers).

use bevy::prelude::*;

use crate::client::interaction_flow::{npc_trade_still_valid, SellerStockSource};
use crate::ui::gameplay::InventoryUiState;
use crate::world::{UnitCatalog, UnitId, WorldData};
use crate::world::relationship::AuthoredRelationshipCatalog;

#[derive(Resource, Debug, Clone, Default)]
pub struct TradeSessionState {
    pub open: bool,
    pub buyer_unit_id: Option<UnitId>,
    pub seller_unit_id: Option<UnitId>,
    /// Legacy accessor — prefer `seller_stock_source`.
    pub seller_inventory_id: Option<crate::world::InventoryId>,
    pub seller_stock_source: Option<SellerStockSource>,
    pub content_revision: u64,
}

impl TradeSessionState {
    pub fn close(&mut self) {
        *self = Self::default();
    }

    pub fn seller_inventory_id(&self) -> Option<crate::world::InventoryId> {
        self.seller_inventory_id
    }

    pub fn is_seller_inventory(&self, inventory_id: crate::world::InventoryId) -> bool {
        self.seller_inventory_id == Some(inventory_id)
    }

    pub fn bump_content_revision(&mut self) {
        self.content_revision += 1;
    }
}

pub fn close_trade_session(trade: &mut TradeSessionState, inventory_ui: &mut InventoryUiState) {
    trade.close();
    inventory_ui.trade_seller_inventory_id = None;
    inventory_ui.invalidate_drag();
}

pub fn reconcile_trade_session(
    trade: &mut TradeSessionState,
    inventory_ui: &mut InventoryUiState,
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
) {
    if !trade.open {
        return;
    }
    let (buyer, seller) = match (trade.buyer_unit_id, trade.seller_unit_id) {
        (Some(b), Some(s)) => (b, s),
        _ => {
            close_trade_session(trade, inventory_ui);
            return;
        }
    };
    if !npc_trade_still_valid(world, authored_relationships, buyer, seller) {
        inventory_ui.feedback_message = "Trade ended.".into();
        close_trade_session(trade, inventory_ui);
    }
}

pub fn seller_display_name(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    seller_unit_id: UnitId,
) -> String {
    crate::client::interaction_flow::unit_display_name(world, unit_catalog, seller_unit_id)
}
