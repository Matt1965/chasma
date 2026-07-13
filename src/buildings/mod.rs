//! Building Runtime Layer (ADR-079 B2).
//!
//! Owns derived, disposable runtime concerns for buildings: placeholder mesh
//! entities and sync with terrain residency. Authoritative instance data remains
//! in [`crate::world::WorldData`].

use bevy::prelude::*;

pub mod components;
pub mod picking;
pub mod placeholder;
pub mod spawn;
pub mod sync;

pub use components::BuildingRenderEntity;
pub use picking::pick_building_along_ray;
pub use spawn::{BuildingRenderAssets, BuildingRenderIndex, spawn_building_render_entity};
pub use sync::BuildingRuntimeSystems;

/// Owns the Building Runtime Layer.
pub struct BuildingsRuntimePlugin;

impl Plugin for BuildingsRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<BuildingRenderEntity>()
            .init_resource::<BuildingRenderIndex>()
            .init_resource::<BuildingRenderAssets>()
            .add_systems(
                Update,
                sync::sync_building_render_entities.in_set(BuildingRuntimeSystems),
            );
    }
}
