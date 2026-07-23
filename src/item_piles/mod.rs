//! Runtime item pile presentation (ADR-090 I4, IA0).

mod assets;
mod components;
#[cfg(feature = "dev")]
mod dev_labels;
mod plugin;
mod presentation;
mod spawn;
mod sync;

pub use assets::{ITEM_ASSET_ROOT, ItemSceneAssets, gltf_asset_path, preload_item_scenes};
pub use components::ItemPileRenderEntity;
pub use presentation::{
    ItemPileFallbackMesh, ItemPileFallbackReason, ItemPilePresentationSettings, ItemPileSceneRoot,
    format_pile_dev_label, pile_display_metadata,
};
pub use spawn::{spawn_item_pile_fallback_entity, spawn_item_pile_scene_entity};
pub use sync::{ItemPileRenderIndex, ItemPileRuntimeSystems, sync_item_pile_render_entities};

use bevy::prelude::*;

pub use plugin::ItemPileRuntimePlugin;
