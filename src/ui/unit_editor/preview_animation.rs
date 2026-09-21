//! Idle animation for Unit Editor preview units (CG3).

use std::time::Duration;

use bevy::prelude::*;

use crate::units::{
    AnimationGraphShareKey, AnimationPlaybackClip, AnimationPlaybackPending, AnimationProfileHandle,
    UnitAnimationAssets, UnitAnimationGraphInstalled, UnitAnimationPlayerLink, UnitAnimationRuntime,
    UnitRenderMetadata, UnitSceneRoot,
};
use crate::units::presentation::{
    UnitEditorPreviewRosterMember, UnitEditorPreviewUnit, UnitPresentationAppearance,
};
use crate::world::{
    AnimationClipKey, AppearanceProfileCatalog, UnitCatalog, effective_render_key_for_appearance,
};

pub fn discover_preview_animation_players(
    mut commands: Commands,
    roots: Query<
        Entity,
        (
            Or<(With<UnitEditorPreviewUnit>, With<UnitEditorPreviewRosterMember>)>,
            With<UnitSceneRoot>,
            Without<UnitAnimationPlayerLink>,
        ),
    >,
    children: Query<&Children>,
    players: Query<Entity, With<AnimationPlayer>>,
) {
    for root in &roots {
        let mut found = Vec::new();
        collect_animation_players(root, &children, &players, &mut found);
        if found.is_empty() {
            continue;
        }
        found.sort_by_key(|entity| entity.to_bits());
        commands.entity(root).insert((
            UnitAnimationPlayerLink {
                player_entity: found[0],
            },
            AnimationPlaybackPending,
        ));
    }
}

pub fn install_preview_animation_graph(
    mut commands: Commands,
    catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    assets: Res<UnitAnimationAssets>,
    roots: Query<
        (
            Entity,
            &UnitRenderMetadata,
            &UnitPresentationAppearance,
            &UnitAnimationPlayerLink,
        ),
        (
            Or<(With<UnitEditorPreviewUnit>, With<UnitEditorPreviewRosterMember>)>,
            Without<UnitAnimationGraphInstalled>,
        ),
    >,
    players: Query<Entity, With<AnimationPlayer>>,
) {
    for (root, metadata, appearance, link) in &roots {
        if players.get(link.player_entity).is_err() {
            continue;
        }
        let Some(definition) = catalog.get(&metadata.definition_id) else {
            continue;
        };
        let render_key = effective_render_key_for_appearance(&appearance.appearance, &appearance_profiles)
            .ok()
            .and_then(|key| key.0.clone())
            .unwrap_or_default();
        let Some(profile_id) = &definition.animation_profile_id else {
            continue;
        };
        let share_key = AnimationGraphShareKey {
            profile_id: profile_id.clone(),
            gltf_asset_path: format!("units/{render_key}.glb"),
        };
        let built = assets
            .graph_for_share_key(&share_key)
            .or_else(|| assets.graph_for(&metadata.definition_id));
        let Some(built) = built else {
            continue;
        };
        commands.entity(link.player_entity).insert((
            AnimationGraphHandle(built.graph.clone()),
            AnimationTransitions::new(),
            UnitAnimationGraphInstalled,
            AnimationProfileHandle {
                profile_id: built.profile_id.clone(),
            },
        ));
        commands
            .entity(root)
            .insert((UnitAnimationGraphInstalled, AnimationPlaybackPending));
    }
}

pub fn sync_preview_idle_animation(
    mut commands: Commands,
    catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    assets: Res<UnitAnimationAssets>,
    roots: Query<
        (
            Entity,
            &UnitAnimationPlayerLink,
            &UnitRenderMetadata,
            &UnitPresentationAppearance,
        ),
        (With<UnitAnimationGraphInstalled>, With<AnimationPlaybackPending>),
    >,
    mut players: Query<(Entity, &mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (root, link, metadata, appearance) in &roots {
        let Some(definition) = catalog.get(&metadata.definition_id) else {
            continue;
        };
        let render_key = effective_render_key_for_appearance(&appearance.appearance, &appearance_profiles)
            .ok()
            .and_then(|key| key.0.clone())
            .unwrap_or_default();
        let share_key = definition.animation_profile_id.as_ref().map(|profile_id| {
            AnimationGraphShareKey {
                profile_id: profile_id.clone(),
                gltf_asset_path: format!("units/{render_key}.glb"),
            }
        });
        let built = share_key
            .as_ref()
            .and_then(|key| assets.graph_for_share_key(key))
            .or_else(|| assets.graph_for(&metadata.definition_id));
        let Some(built) = built else {
            continue;
        };
        let Some(node) = built.locomotion_nodes.get(&AnimationClipKey::Idle) else {
            continue;
        };
        if let Ok((_, mut player, mut transitions)) = players.get_mut(link.player_entity) {
            transitions
                .play(&mut player, *node, Duration::ZERO)
                .repeat();
            let clip = AnimationPlaybackClip::Locomotion(AnimationClipKey::Idle);
            commands.entity(root).insert((
                UnitAnimationRuntime {
                    current_clip: clip,
                    layers: Default::default(),
                },
                AnimationProfileHandle {
                    profile_id: built.profile_id.clone(),
                },
            ));
            commands.entity(root).remove::<AnimationPlaybackPending>();
        }
    }
}

fn collect_animation_players(
    root: Entity,
    children: &Query<&Children>,
    players: &Query<Entity, With<AnimationPlayer>>,
    found: &mut Vec<Entity>,
) {
    if players.get(root).is_ok() {
        found.push(root);
    }
    if let Ok(kids) = children.get(root) {
        for child in kids.iter() {
            collect_animation_players(child, children, players, found);
        }
    }
}
