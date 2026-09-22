use bevy::prelude::*;

use crate::player::RuntimeSyncSystems;
use crate::terrain::TerrainStreamingSystems;

use super::components::{
    CorpseDeathPosePending, CorpsePresentationClaim, CorpseRenderEntity, CorpseSceneRoot,
};
use super::death_pose::begin_corpse_death_poses;
use super::sync::{CorpseRenderIndex, CorpseRuntimeSystems, sync_corpse_render_entities};

/// Registers corpse runtime presentation.
pub struct CorpseRuntimePlugin;

impl Plugin for CorpseRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CorpseRenderEntity>()
            .register_type::<CorpseSceneRoot>()
            .register_type::<CorpsePresentationClaim>()
            .register_type::<CorpseDeathPosePending>()
            .init_resource::<CorpseRenderIndex>()
            .add_systems(
                Update,
                (
                    sync_corpse_render_entities,
                    begin_corpse_death_poses.after(sync_corpse_render_entities),
                )
                    .chain()
                    .in_set(CorpseRuntimeSystems),
            )
            .configure_sets(
                Update,
                CorpseRuntimeSystems
                    .after(TerrainStreamingSystems)
                    .in_set(RuntimeSyncSystems),
            );
    }
}
