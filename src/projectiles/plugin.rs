use bevy::prelude::*;

use super::assets::ProjectileSceneAssets;
use super::components::{ProjectileRenderEntity, ProjectileSceneRoot};
use super::spawn::ProjectileRenderIndex;
use super::sync::{ProjectileRuntimeSystems, sync_projectile_render_entities};

/// Owns the Projectile Runtime Layer (ADR-060 C7).
pub struct ProjectilesRuntimePlugin;

impl Plugin for ProjectilesRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ProjectileRenderEntity>()
            .register_type::<ProjectileSceneRoot>()
            .init_resource::<ProjectileRenderIndex>()
            .init_resource::<ProjectileSceneAssets>()
            .add_systems(
                Update,
                sync_projectile_render_entities.in_set(ProjectileRuntimeSystems),
            );
    }
}
