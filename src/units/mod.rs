//! Unit Runtime Layer (ADR-028).
//!
//! Owns derived, disposable runtime concerns for units: glTF asset handles,
//! ECS render entities, and sync with terrain residency. Authoritative instance
//! data remains in [`crate::world::WorldData`].

mod assets;
mod components;
#[cfg(feature = "dev")]
mod dev_spawn;
pub mod input;
mod plugin;
mod settings;
mod spawn;
mod sync;

pub use input::{
    cursor_screen_position, cursor_world_ray, pick_unit_along_ray,
    terrain_click_to_world_position, unit_pick_radius, world_position_to_screen, BoxSelectDrag,
    PlayerInteractionSettings, SelectedUnits, TerrainClickResult,
};

pub use assets::{gltf_asset_path, preload_unit_scenes, UnitSceneAssets, UNIT_ASSET_ROOT};
pub use components::{UnitRenderEntity, UnitSceneRoot, UnitSelectionIndicator};
pub use plugin::UnitsRuntimePlugin;
pub use settings::UnitsRuntimeSettings;
pub use spawn::{despawn_unit_render_entities, spawn_unit_render_entity, UnitRenderIndex};
pub use sync::{sync_unit_render_entities, UnitRuntimeSystems, UnitSyncOverrides};
