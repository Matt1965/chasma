use bevy::prelude::*;

use crate::world::SettlementAnchorId;

#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct SettlementAnchorRenderEntity {
    pub anchor_id: SettlementAnchorId,
}
