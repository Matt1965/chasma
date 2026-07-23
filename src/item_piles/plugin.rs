use bevy::prelude::*;

use super::assets::{ItemSceneAssets, preload_item_scenes};
use super::components::ItemPileRenderEntity;
use super::presentation::{ItemPileFallbackAssets, ItemPilePresentationSettings};
use super::sync::{ItemPileRenderIndex, ItemPileRuntimeSystems, sync_item_pile_render_entities};
use crate::player::RuntimeSyncSystems;

#[cfg(feature = "dev")]
use super::dev_labels::{
    billboard_item_pile_dev_labels, sync_item_pile_dev_labels, ItemPileDevLabelIndex,
};

/// Registers item pile runtime presentation (ADR-090 I4, IA0).
pub struct ItemPileRuntimePlugin;

impl Plugin for ItemPileRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ItemPileRenderEntity>()
            .register_type::<ItemPilePresentationSettings>()
            .register_type::<super::presentation::ItemPileFallbackMesh>()
            .register_type::<super::presentation::ItemPileFallbackReason>()
            .register_type::<super::presentation::ItemPileSceneRoot>()
            .init_resource::<ItemPileRenderIndex>()
            .init_resource::<ItemPileFallbackAssets>()
            .init_resource::<ItemPilePresentationSettings>()
            .add_systems(Startup, init_item_scene_assets)
            .add_systems(
                Update,
                sync_item_pile_render_entities.in_set(ItemPileRuntimeSystems),
            );

        #[cfg(feature = "dev")]
        {
            app.init_resource::<ItemPileDevLabelIndex>()
                .add_systems(
                    Update,
                    (
                        sync_item_pile_dev_labels.after(sync_item_pile_render_entities),
                        billboard_item_pile_dev_labels.after(sync_item_pile_dev_labels),
                    )
                        .in_set(ItemPileRuntimeSystems),
                );
        }

        app.configure_sets(
            Update,
            ItemPileRuntimeSystems
                .after(crate::terrain::TerrainStreamingSystems)
                .in_set(RuntimeSyncSystems),
        );
    }
}

fn init_item_scene_assets(
    catalog: Res<crate::world::ItemCatalog>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.insert_resource(preload_item_scenes(&catalog, &asset_server));
}
