use bevy::prelude::*;

use super::components::SettlementAnchorRenderEntity;
use super::spawn::SettlementAnchorRenderIndex;
use super::sync::{SettlementAnchorRuntimeSystems, sync_settlement_anchor_render_entities};
use crate::player::RuntimeSyncSystems;

pub struct SettlementAnchorRuntimePlugin;

impl Plugin for SettlementAnchorRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SettlementAnchorRenderEntity>()
            .init_resource::<SettlementAnchorRenderIndex>()
            .add_systems(
                Update,
                sync_settlement_anchor_render_entities.in_set(SettlementAnchorRuntimeSystems),
            )
            .configure_sets(
                Update,
                SettlementAnchorRuntimeSystems
                    .after(crate::terrain::TerrainStreamingSystems)
                    .in_set(RuntimeSyncSystems),
            );
    }
}
