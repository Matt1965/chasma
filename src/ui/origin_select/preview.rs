//! Multi-actor origin roster preview spawn (CG7/CG8).

use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::camera::render_layers::PREVIEW_RENDER_LAYER;
use crate::menu::StartingSquadSession;
use crate::units::{
    UnitAppearanceMorphFingerprint, UnitRenderMetadata, UnitSceneAssets, UnitSceneRoot,
    presentation::{UnitEditorPreviewRosterMember, UnitEditorPreviewRoot, UnitPresentationAppearance},
};
use crate::world::{
    AppearanceProfileCatalog, OriginCatalog, UnitCatalog, effective_render_key_for_appearance,
    unit_visual_scale,
};

use super::preview_spawn::appearance_requires_preview_respawn;

pub fn sync_origin_select_preview_roster(
    mut commands: Commands,
    session: Res<StartingSquadSession>,
    origins: Res<OriginCatalog>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<UnitSceneAssets>,
    preview_roots: Query<Entity, With<UnitEditorPreviewRoot>>,
    existing: Query<(Entity, &UnitEditorPreviewRosterMember, &UnitPresentationAppearance)>,
    mut last_origin_index: Local<Option<usize>>,
) {
    let Some(draft) = session.active_draft(&origins) else {
        return;
    };
    let Some(parent) = preview_roots.iter().next() else {
        return;
    };

    if last_origin_index
        .map(|value| value != session.selected_origin_index)
        .unwrap_or(true)
    {
        for (entity, _, _) in &existing {
            commands.entity(entity).despawn();
        }
        *last_origin_index = Some(session.selected_origin_index);
    }

    let present: HashSet<usize> = existing
        .iter()
        .map(|(_, member, _)| member.slot_index)
        .collect();

    for (slot_index, member) in draft.members.iter().enumerate() {
        let Some(definition) = unit_catalog.get(&member.definition_id) else {
            continue;
        };
        let appearance = member.appearance.appearance.clone();
        let render_key = match effective_render_key_for_appearance(&appearance, &appearance_profiles) {
            Ok(key) => key,
            Err(_) => continue,
        };
        let render_key_str = render_key.0.as_deref().unwrap_or("");

        if let Some((entity, _, current)) = existing
            .iter()
            .find(|(_, roster, _)| roster.slot_index == slot_index)
        {
            if current.appearance == appearance {
                continue;
            }
            if appearance_requires_preview_respawn(&current.appearance, &appearance) {
                commands.entity(entity).despawn();
            } else {
                let visual_scale = unit_visual_scale(definition, appearance.height_scale);
                commands.entity(entity).insert((
                    UnitPresentationAppearance { appearance },
                    Transform::from_translation(member.preview_offset).with_scale(visual_scale),
                ));
                commands.entity(entity).remove::<UnitAppearanceMorphFingerprint>();
                continue;
            }
        } else if present.contains(&slot_index) {
            continue;
        }

        let Some(scene) = scene_assets.scene_for_render_key(render_key_str).cloned() else {
            if !render_key_str.is_empty() {
                scene_assets.log_missing_once(render_key_str);
            }
            continue;
        };
        if !matches!(asset_server.get_load_state(&scene), Some(LoadState::Loaded)) {
            continue;
        }
        let visual_scale = unit_visual_scale(definition, appearance.height_scale);
        commands.entity(parent).with_children(|parent| {
            parent.spawn((
                UnitEditorPreviewRosterMember { slot_index },
                UnitPresentationAppearance { appearance },
                UnitRenderMetadata {
                    definition_id: member.definition_id.clone(),
                },
                UnitSceneRoot,
                SceneRoot(scene),
                Transform::from_translation(member.preview_offset).with_scale(visual_scale),
                Visibility::default(),
                PREVIEW_RENDER_LAYER,
            ));
        });
    }
}
