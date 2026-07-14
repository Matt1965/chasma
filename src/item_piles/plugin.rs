use bevy::prelude::*;

use super::components::ItemPileRenderEntity;
use super::sync::{ItemPileRenderIndex, ItemPileRuntimeSystems, sync_item_pile_render_entities};
use crate::player::RuntimeSyncSystems;

/// Registers item pile runtime presentation (ADR-090 I4).
pub struct ItemPileRuntimePlugin;

impl Plugin for ItemPileRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ItemPileRenderEntity>()
            .init_resource::<ItemPileRenderIndex>()
            .add_systems(
                Update,
                sync_item_pile_render_entities.in_set(ItemPileRuntimeSystems),
            )
            .configure_sets(
                Update,
                ItemPileRuntimeSystems
                    .after(crate::terrain::TerrainStreamingSystems)
                    .in_set(RuntimeSyncSystems),
            );
    }
}
