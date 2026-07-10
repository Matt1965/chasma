use bevy::prelude::*;

use crate::world::ProjectileId;

/// Marks a disposable ECS mirror of an authoritative [`ProjectileRecord`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct ProjectileRenderEntity {
    pub projectile_id: ProjectileId,
}

/// Root marker for a loaded projectile glTF scene instance.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
pub struct ProjectileSceneRoot;
