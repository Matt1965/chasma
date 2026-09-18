//! Multi-actor origin roster preview spawn (CG7).

use std::collections::HashSet;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::camera::render_layers::PREVIEW_RENDER_LAYER;
use crate::units::{
    UnitRenderMetadata, UnitSceneAssets, UnitSceneRoot,
    presentation::{UnitEditorPreviewRosterMember, UnitEditorPreviewRoot, UnitPresentationAppearance},
};
use crate::world::{
    AppearanceProfileCatalog, OriginCatalog, UnitCatalog, effective_render_key_for_appearance,
    resolve_canonical_default_appearance, unit_visual_scale,
};

use super::session::OriginSelectSession;

pub fn sync_origin_select_preview_roster(
    mut commands: Commands,
    session: Res<OriginSelectSession>,
    origins: Res<OriginCatalog>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<UnitSceneAssets>,
    preview_roots: Query<Entity, With<UnitEditorPreviewRoot>>,
    existing: Query<(Entity, &UnitEditorPreviewRosterMember)>,
    mut last_index: Local<Option<usize>>,
) {
    let Some(origin) = origins.get_index(session.selected_index) else {
        return;
    };
    let Some(parent) = preview_roots.iter().next() else {
        return;
    };

    if last_index.map(|value| value != session.selected_index).unwrap_or(true) {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        *last_index = Some(session.selected_index);
    }

    let present: HashSet<usize> = existing.iter().map(|(_, member)| member.slot_index).collect();

    for (slot_index, member) in origin.roster.iter().enumerate() {
        if present.contains(&slot_index) {
            continue;
        }
        let Some(definition) = unit_catalog.get(&member.definition_id) else {
            warn!(
                "origin preview skipped missing definition `{}`",
                member.definition_id.as_str()
            );
            continue;
        };
        let appearance = match resolve_canonical_default_appearance(definition, &appearance_profiles)
        {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    "origin preview appearance failed for `{}`: {error}",
                    member.definition_id.as_str()
                );
                continue;
            }
        };
        let render_key = match effective_render_key_for_appearance(&appearance, &appearance_profiles)
        {
            Ok(key) => key,
            Err(error) => {
                warn!("origin preview render key failed: {error}");
                continue;
            }
        };
        let render_key_str = render_key.0.as_deref().unwrap_or("");
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
