//! Unit Runtime Layer (ADR-028).
//!
//! Owns derived, disposable runtime concerns for units: glTF asset handles,
//! ECS render entities, and sync with terrain residency. Authoritative instance
//! data remains in [`crate::world::WorldData`].

mod assets;
mod components;
mod plugin;
mod settings;
mod spawn;
mod sync;

pub use assets::{gltf_asset_path, preload_unit_scenes, UnitSceneAssets, UNIT_ASSET_ROOT};
pub use components::{UnitRenderEntity, UnitSceneRoot};
pub use plugin::UnitsRuntimePlugin;
pub use settings::UnitsRuntimeSettings;
pub use spawn::{despawn_unit_render_entities, spawn_unit_render_entity, UnitRenderIndex};
pub use sync::{sync_unit_render_entities, UnitRuntimeSystems, UnitSyncOverrides};
