use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::TerrainRenderAssets;
use crate::units::{UnitPresentationAppearance, UnitSceneAssets, UnitSyncOverrides};
use crate::world::{
    AppearanceProfileCatalog, CorpseId, CorpseState, UnitCatalog, WorldConfig, WorldData,
};

use crate::units::{DeathPresentation, UnitRenderEntity};

use super::components::{CorpsePresentationClaim, CorpseRenderEntity};

#[derive(SystemParam)]
pub(crate) struct CorpseSyncQueries<'w, 's> {
    existing: Query<'w, 's, (Entity, &'static CorpseRenderEntity, &'static Transform)>,
    entities: Query<'w, 's, Entity>,
    claims: Query<'w, 's, &'static CorpsePresentationClaim>,
    death_presentations: Query<'w, 's, &'static DeathPresentation>,
    corpse_render_entities: Query<'w, 's, &'static CorpseRenderEntity>,
    unit_render_roots: Query<'w, 's, &'static UnitRenderEntity>,
}

#[derive(SystemParam)]
pub(crate) struct CorpseSyncRenderContext<'w> {
    render_assets: Option<Res<'w, TerrainRenderAssets>>,
    overrides: Option<Res<'w, UnitSyncOverrides>>,
}
use super::ownership::{
    corpse_origin_has_pending_death_root, release_stale_corpse_presentation_owners,
    should_spawn_corpse_presentation,
};
use super::spawn::{despawn_corpse_render_entities, spawn_corpse_render_entity};

/// Index of corpse render entities.
#[derive(Resource, Default, Debug)]
pub struct CorpseRenderIndex(pub std::collections::HashMap<CorpseId, Entity>);

/// Systems that sync corpse render entities.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct CorpseRuntimeSystems;

pub(crate) fn visible_corpse_ids(
    world: &WorldData,
    residency: &ChunkResidencyTracker,
) -> HashSet<CorpseId> {
    let mut visible = HashSet::new();
    for corpse_id in world.corpse_store().sorted_corpse_ids() {
        let Some(chunk) = world.corpse_store().corpse_chunk(corpse_id) else {
            continue;
        };
        let Some(record) = world.corpse_store().get(corpse_id) else {
            continue;
        };
        if record.state != CorpseState::Present {
            continue;
        }
        if residency.is_resident(chunk) {
            visible.insert(corpse_id);
        }
    }
    visible
}

/// Keep derived corpse entities aligned with [`WorldData`] chunk residency.
pub(crate) fn sync_corpse_render_entities(
    mut commands: Commands,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    config: Res<WorldConfig>,
    residency: Res<ChunkResidencyTracker>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<UnitSceneAssets>,
    mut index: ResMut<CorpseRenderIndex>,
    queries: CorpseSyncQueries,
    render: CorpseSyncRenderContext,
) {
    release_stale_corpse_presentation_owners(
        &mut index,
        &queries.entities,
        &queries.claims,
        &queries.death_presentations,
        &queries.corpse_render_entities,
    );

    let vertical_scale = render
        .render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let force_scenes_loaded = render
        .overrides
        .as_ref()
        .is_some_and(|value| value.treat_scenes_loaded);
    let should_render = visible_corpse_ids(&world, &residency);

    let stale: Vec<CorpseId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !should_render.contains(id))
        .collect();
    despawn_corpse_render_entities(&mut commands, &mut index, stale);

    for (entity, marker, transform) in &queries.existing {
        if !should_render.contains(&marker.corpse_id) {
            continue;
        }
        let Some(record) = world.corpse_store().get(marker.corpse_id) else {
            commands.entity(entity).despawn();
            index.0.remove(&marker.corpse_id);
            continue;
        };
        if record.state != CorpseState::Present {
            commands.entity(entity).despawn();
            index.0.remove(&marker.corpse_id);
            continue;
        }
        let Some(definition) = catalog.get(&record.unit_definition_id) else {
            continue;
        };
        let height_scale = record
            .appearance
            .as_ref()
            .map(|appearance| appearance.height_scale)
            .unwrap_or(1.0);
        let render_scale = crate::world::unit_visual_scale(definition, height_scale);
        let layout = config.chunk_layout();
        let translation =
            super::spawn::corpse_render_translation(&world, record, layout, vertical_scale);
        commands.entity(entity).insert(Transform {
            translation,
            rotation: transform.rotation,
            scale: render_scale,
        });
        if let Some(appearance) = record.appearance.as_ref() {
            commands
                .entity(entity)
                .insert(UnitPresentationAppearance {
                    appearance: appearance.clone(),
                });
        }
    }

    for corpse_id in should_render {
        let Some(record) = world.corpse_store().get(corpse_id) else {
            continue;
        };
        let origin_pending = corpse_origin_has_pending_death_root(
            &world,
            record.origin_unit_id,
            &queries.unit_render_roots,
            &queries.death_presentations,
        );
        if !should_spawn_corpse_presentation(&world, corpse_id, &index, origin_pending) {
            continue;
        }
        let Some(definition) = catalog.get(&record.unit_definition_id) else {
            warn!(
                "corpse {} references missing definition `{}`",
                record.id.raw(),
                record.unit_definition_id.as_str()
            );
            continue;
        };
        let render_key = corpse_render_key(record, definition, &appearance_profiles);
        let render_key_str = render_key.0.as_deref().unwrap_or("");
        let Some(scene) = scene_assets.scene_for_render_key(render_key_str).cloned() else {
            if !render_key_str.is_empty() {
                scene_assets.log_missing_once(render_key_str);
            }
            continue;
        };
        if !force_scenes_loaded && !scene_is_loaded(&asset_server, &scene) {
            continue;
        }
        let height_scale = record
            .appearance
            .as_ref()
            .map(|appearance| appearance.height_scale)
            .unwrap_or(1.0);
        let visual_scale = crate::world::unit_visual_scale(definition, height_scale);
        let entity = spawn_corpse_render_entity(
            &mut commands,
            &world,
            record,
            definition,
            scene,
            &config,
            vertical_scale,
            visual_scale,
            true,
        );
        if let Some(appearance) = record.appearance.as_ref() {
            commands.entity(entity).insert(UnitPresentationAppearance {
                appearance: appearance.clone(),
            });
        }
        index.0.insert(corpse_id, entity);
    }
}

fn corpse_render_key(
    record: &crate::world::CorpseRecord,
    definition: &crate::world::UnitDefinition,
    appearance_profiles: &AppearanceProfileCatalog,
) -> crate::world::UnitRenderKey {
    if let Some(appearance) = record.appearance.as_ref() {
        if let Ok(key) =
            crate::world::effective_render_key_for_appearance(appearance, appearance_profiles)
        {
            return key;
        }
    }
    definition.render_key.clone()
}

fn scene_is_loaded(asset_server: &AssetServer, scene: &Handle<Scene>) -> bool {
    matches!(asset_server.get_load_state(scene), Some(LoadState::Loaded))
}
