//! Unit Runtime Layer (ADR-028).
//!
//! Owns derived, disposable runtime concerns for units: glTF asset handles,
//! ECS render entities, and sync with terrain residency. Authoritative instance
//! data remains in [`crate::world::WorldData`].

mod animation;
mod assets;
mod components;
#[cfg(feature = "dev")]
mod dev_spawn;
mod health_bars;
pub mod input;
mod plugin;
mod settings;
mod spawn;
mod sync;

pub use input::{
    BoxSelectDrag, PlayerInteractionSettings, SelectedUnits, TerrainClickResult,
    cursor_screen_position, cursor_world_ray, pick_unit_along_ray, terrain_click_to_world_position,
    unit_pick_radius, world_position_to_screen,
};

pub use animation::{
    AnimationPresentationFocus, AnimationPresentationMetrics, UnitAnimationAssets,
    UnitAnimationIntent, UnitAnimationPlayerLink, UnitAnimationPlugin, UnitAnimationRuntime,
    UnitAnimationSettings, UnitAnimationStateIndex, UnitAnimationSystems, ValidationSeverity,
    derive_unit_animation_intent, locomotion_debug_snapshot,
};
pub use assets::{UNIT_ASSET_ROOT, UnitSceneAssets, gltf_asset_path, preload_unit_scenes};
pub use components::{UnitRenderEntity, UnitRenderMetadata, UnitSceneRoot, UnitSelectionIndicator};
pub use health_bars::{
    UnitHealthBar, UnitHealthBarState, billboard_unit_health_bars, health_bar_color,
    health_percent, should_show_health_bar, sync_unit_health_bars,
};
pub use plugin::UnitsRuntimePlugin;
pub use settings::UnitsRuntimeSettings;
pub use spawn::{UnitRenderIndex, despawn_unit_render_entities, spawn_unit_render_entity};
pub use sync::{UnitRuntimeSystems, UnitSyncOverrides, sync_unit_render_entities};
