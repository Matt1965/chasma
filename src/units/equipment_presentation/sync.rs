//! Sync equipment visuals from authoritative inventories to unit render hierarchies.

use std::collections::{HashMap, HashSet};

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::item_piles::ItemSceneAssets;
use crate::units::components::{UnitRenderEntity, UnitRenderMetadata, UnitSceneRoot};
use crate::units::spawn::UnitRenderIndex;
use crate::units::sync::UnitSyncOverrides;
use crate::world::equipment::EquipmentPresentationMode;
use crate::world::{
    AppearanceProfileCatalog, EquipmentVisualCatalog, ItemCatalog, UnitCatalog, UnitId, WorldData,
    effective_unit_render_key_str,
};

use super::bones::resolve_socket_bone_entity;
use super::components::{
    UnitEquipmentMorphConfig, UnitEquipmentSceneRoot, UnitEquipmentSkinnedPending,
    UnitEquipmentVisual,
};
use super::resolve::desired_equipment_presentations_for_unit;

/// Tracks spawned equipment visual entities per unit slot.
#[derive(Resource, Default, Debug)]
pub struct UnitEquipmentPresentationIndex {
    pub visuals: HashMap<(UnitId, crate::world::EquipmentSlot), Entity>,
    pub instance_ids: HashMap<(UnitId, crate::world::EquipmentSlot), crate::world::ItemInstanceId>,
    pub equipped_render_keys: HashMap<(UnitId, crate::world::EquipmentSlot), String>,
    pub presentation_sockets:
        HashMap<(UnitId, crate::world::EquipmentSlot), crate::world::equipment::EquipmentAttachmentSocket>,
}

/// Reconcile equipment visuals for all currently rendered units.
pub fn sync_unit_equipment_presentation(
    mut commands: Commands,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    items: Res<ItemCatalog>,
    visuals: Res<EquipmentVisualCatalog>,
    index: Res<UnitRenderIndex>,
    mut state: ResMut<UnitEquipmentPresentationIndex>,
    mut scene_assets: ResMut<ItemSceneAssets>,
    asset_server: Res<AssetServer>,
    overrides: Option<Res<UnitSyncOverrides>>,
    unit_roots: Query<(
        Entity,
        &UnitRenderEntity,
        &UnitRenderMetadata,
        &UnitSceneRoot,
    )>,
    visuals_alive: Query<Entity, With<UnitEquipmentVisual>>,
    names: Query<&Name>,
    child_of: Query<&ChildOf>,
    children: Query<&Children>,
) {
    let force_loaded = overrides
        .as_ref()
        .is_some_and(|value| value.treat_scenes_loaded);
    let mut desired_keys: HashSet<(UnitId, crate::world::EquipmentSlot)> = HashSet::new();

    for (render_entity, marker, metadata, _) in &unit_roots {
        if index.0.get(&marker.unit_id) != Some(&render_entity) {
            continue;
        }
        let Some(unit) = world.get_unit(marker.unit_id) else {
            continue;
        };
        let Some(definition) = unit_catalog.get(&metadata.definition_id) else {
            continue;
        };
        let unit_render_key = match effective_unit_render_key_str(
            unit,
            definition,
            &appearance_profiles,
        ) {
            Ok(key) => key,
            Err(error) => {
                warn!(
                    "equipment presentation skipped for unit {}: {error}",
                    marker.unit_id.raw()
                );
                continue;
            }
        };

        let desired = desired_equipment_presentations_for_unit(
            world.as_ref(),
            items.as_ref(),
            visuals.as_ref(),
            &unit_render_key,
            unit,
        );
        for entry in desired {
            desired_keys.insert((entry.unit_id, entry.slot));
            let key = (entry.unit_id, entry.slot);
            let equipped_render_key = entry.render_key.0.clone().unwrap_or_default();
            let needs_respawn = state.instance_ids.get(&key) != Some(&entry.item_instance_id)
                || state.equipped_render_keys.get(&key) != Some(&equipped_render_key)
                || state.presentation_sockets.get(&key) != Some(&entry.presentation.socket)
                || state
                    .visuals
                    .get(&key)
                    .is_none_or(|entity| visuals_alive.get(*entity).is_err());
            if !needs_respawn {
                continue;
            }
            if let Some(old) = state.visuals.remove(&key) {
                if visuals_alive.get(old).is_ok() {
                    commands.entity(old).despawn();
                }
            }
            state.instance_ids.remove(&key);
            state.equipped_render_keys.remove(&key);

            let Some(render_key_ref) = entry.render_key.0.as_ref() else {
                continue;
            };
            let Some(scene) = scene_assets.ensure_scene(render_key_ref, asset_server.as_ref())
            else {
                scene_assets.log_missing_once(render_key_ref);
                continue;
            };
            if !force_loaded
                && !matches!(asset_server.get_load_state(&scene), Some(LoadState::Loaded))
            {
                continue;
            }

            match entry.mode {
                EquipmentPresentationMode::RigidAttachment => {
                    let socket = entry.presentation.socket;
                    let Some(bone) = resolve_socket_bone_entity(
                        render_entity,
                        &unit_render_key,
                        socket,
                        &names,
                        &child_of,
                        &children,
                    ) else {
                        warn!(
                            "equipment socket {:?} unresolved for unit `{}` render key `{}`",
                            socket,
                            marker.unit_id.raw(),
                            unit_render_key
                        );
                        continue;
                    };
                    let visual = commands
                        .spawn((
                            UnitEquipmentVisual {
                                unit_id: entry.unit_id,
                                slot: entry.slot,
                                item_instance_id: entry.item_instance_id,
                            },
                            UnitEquipmentSceneRoot,
                            SceneRoot(scene),
                            Transform {
                                translation: entry.presentation.local_translation,
                                rotation: entry.presentation.local_rotation,
                                scale: entry.presentation.local_scale,
                            },
                            Visibility::default(),
                            ChildOf(bone),
                        ))
                        .id();
                    state.visuals.insert(key, visual);
                }
                EquipmentPresentationMode::SkinnedOverlay => {
                    // Skinned fit_scale/fit_offset are baked offline; do not scale the overlay root.
                    let consumed_morph_params = visuals
                        .resolve(&entry.item_definition_id, &unit_render_key)
                        .map(|mapping| mapping.consumed_morph_params.clone())
                        .unwrap_or_default();
                    let visual = commands
                        .spawn((
                            UnitEquipmentVisual {
                                unit_id: entry.unit_id,
                                slot: entry.slot,
                                item_instance_id: entry.item_instance_id,
                            },
                            UnitEquipmentMorphConfig {
                                consumed_morph_params,
                            },
                            UnitEquipmentSceneRoot,
                            UnitEquipmentSkinnedPending,
                            SceneRoot(scene),
                            Transform {
                                translation: entry.presentation.local_translation,
                                rotation: entry.presentation.local_rotation,
                                scale: entry.presentation.local_scale,
                            },
                            Visibility::default(),
                            ChildOf(render_entity),
                        ))
                        .id();
                    state.visuals.insert(key, visual);
                }
            }
            state.instance_ids.insert(key, entry.item_instance_id);
            state.equipped_render_keys.insert(key, equipped_render_key);
            state
                .presentation_sockets
                .insert(key, entry.presentation.socket);
        }
    }

    let stale: Vec<(UnitId, crate::world::EquipmentSlot)> = state
        .visuals
        .keys()
        .filter(|key| !desired_keys.contains(key) || !index.0.contains_key(&key.0))
        .copied()
        .collect();
    for key in stale {
        if let Some(entity) = state.visuals.remove(&key) {
            if visuals_alive.get(entity).is_ok() {
                commands.entity(entity).despawn();
            }
        }
        state.instance_ids.remove(&key);
        state.equipped_render_keys.remove(&key);
    }
}
