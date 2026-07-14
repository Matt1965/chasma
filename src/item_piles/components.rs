use bevy::prelude::*;

use crate::world::ItemPileId;

/// Maps an authoritative item pile to its render entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct ItemPileRenderEntity {
    pub pile_id: ItemPileId,
}
