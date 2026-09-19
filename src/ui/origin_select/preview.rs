//! Multi-actor origin roster preview spawn (CG7/CG8).

use std::collections::{HashMap, HashSet};

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::camera::render_layers::PREVIEW_RENDER_LAYER;
use crate::menu::{SquadMemberDraftId, StartingSquadSession};
use crate::ui::unit_editor::{UnitEditorMode, UnitEditorSession};
use crate::units::{
    UnitAppearanceMorphFingerprint, UnitRenderMetadata, UnitSceneAssets, UnitSceneRoot,
    presentation::{UnitEditorPreviewRosterMember, UnitEditorPreviewRoot, UnitPresentationAppearance},
};
use crate::world::{
    AppearanceProfileCatalog, OriginCatalog, UnitAppearance, UnitCatalog,
    effective_render_key_for_appearance, unit_visual_scale,
};

use super::preview_reconcile::{duplicate_roster_actor_entities, orphan_roster_actor_entities};
use super::preview_spawn::appearance_requires_preview_respawn;

fn effective_member_appearance(
    member: &crate::menu::SquadMemberDraft,
    slot_index: usize,
    session: &StartingSquadSession,
    editor_session: Option<&UnitEditorSession>,
) -> UnitAppearance {
    if session.focused_slot_index() == Some(slot_index) {
        if let Some(editor) = editor_session {
            if matches!(
                editor.mode,
                UnitEditorMode::NewGameDraft {
                    slot_index: focused_slot
                } if focused_slot == slot_index
            ) {
                return editor.draft.appearance.clone();
            }
        }
    }
    member.appearance.appearance.clone()
}

fn should_sync_member_appearance(
    slot_index: usize,
    focused: Option<usize>,
) -> bool {
    match focused {
        None => true,
        Some(focused_slot) => focused_slot == slot_index,
    }
}

/// Despawn any CG3 single-actor preview entities that leaked onto the origin stage.
pub fn cleanup_stray_unit_editor_preview_actors(
    mut commands: Commands,
    stray: Query<Entity, With<crate::units::presentation::UnitEditorPreviewUnit>>,
) {
    for entity in &stray {
        commands.entity(entity).despawn();
    }
}

pub fn sync_origin_select_preview_roster(
    mut commands: Commands,
    session: Res<StartingSquadSession>,
    editor_session: Option<Res<UnitEditorSession>>,
    origins: Res<OriginCatalog>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<UnitSceneAssets>,
    preview_roots: Query<Entity, With<UnitEditorPreviewRoot>>,
    existing: Query<(Entity, &UnitEditorPreviewRosterMember, &UnitPresentationAppearance)>,
    mut last_origin_index: Local<Option<usize>>,
    mut pending_respawn: Local<HashSet<SquadMemberDraftId>>,
) {
    let Some(draft) = session.active_draft(&origins) else {
        return;
    };
    let Some(parent) = preview_roots.iter().next() else {
        return;
    };
    let focused = session.focused_slot_index();

    if last_origin_index
        .map(|value| value != session.selected_origin_index)
        .unwrap_or(true)
    {
        for (entity, _, _) in &existing {
            commands.entity(entity).despawn();
        }
        pending_respawn.clear();
        *last_origin_index = Some(session.selected_origin_index);
        return;
    }

    let roster_snapshot: Vec<(Entity, SquadMemberDraftId, usize, UnitAppearance)> = existing
        .iter()
        .map(|(entity, member, appearance)| {
            (
                entity,
                member.draft_member_id,
                member.slot_index,
                appearance.appearance.clone(),
            )
        })
        .collect();

    let active_ids: HashSet<SquadMemberDraftId> =
        draft.members.iter().map(|member| member.id).collect();

    for entity in duplicate_roster_actor_entities(
        &roster_snapshot
            .iter()
            .map(|(entity, id, _, _)| (*entity, *id))
            .collect::<Vec<_>>(),
    ) {
        commands.entity(entity).despawn();
    }
    for entity in orphan_roster_actor_entities(
        &roster_snapshot
            .iter()
            .map(|(entity, id, _, _)| (*entity, *id))
            .collect::<Vec<_>>(),
        &active_ids,
    ) {
        commands.entity(entity).despawn();
    }

    let mut actors_by_draft_id: HashMap<SquadMemberDraftId, (Entity, usize, UnitAppearance)> =
        HashMap::new();
    for (entity, member, appearance) in &existing {
        if !active_ids.contains(&member.draft_member_id) {
            continue;
        }
        match actors_by_draft_id.get(&member.draft_member_id) {
            Some((kept, _, _)) if kept.to_bits() <= entity.to_bits() => {}
            _ => {
                actors_by_draft_id.insert(
                    member.draft_member_id,
                    (entity, member.slot_index, appearance.appearance.clone()),
                );
            }
        }
    }

    pending_respawn.retain(|draft_member_id| actors_by_draft_id.contains_key(draft_member_id));

    for (slot_index, member) in draft.members.iter().enumerate() {
        let appearance = effective_member_appearance(
            member,
            slot_index,
            &session,
            editor_session.as_deref(),
        );
        let Some(definition) = unit_catalog.get(&member.definition_id) else {
            continue;
        };
        let render_key = match effective_render_key_for_appearance(&appearance, &appearance_profiles) {
            Ok(key) => key,
            Err(_) => continue,
        };
        let render_key_str = render_key.0.as_deref().unwrap_or("");

        if let Some((entity, _, current)) = actors_by_draft_id.get(&member.id) {
            if current == &appearance {
                continue;
            }
            if !should_sync_member_appearance(slot_index, focused) {
                continue;
            }
            if appearance_requires_preview_respawn(current, &appearance) {
                if focused.is_some() {
                    continue;
                }
                commands.entity(*entity).despawn();
                actors_by_draft_id.remove(&member.id);
                pending_respawn.insert(member.id);
                continue;
            }
            commands
                .entity(*entity)
                .insert(UnitPresentationAppearance { appearance });
            commands.entity(*entity).remove::<UnitAppearanceMorphFingerprint>();
            continue;
        }

        if focused.is_some() || pending_respawn.contains(&member.id) {
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
                UnitEditorPreviewRosterMember {
                    draft_member_id: member.id,
                    slot_index,
                },
                UnitPresentationAppearance { appearance },
                UnitRenderMetadata {
                    definition_id: member.definition_id.clone(),
                },
                UnitSceneRoot,
                SceneRoot(scene),
                Transform::from_scale(visual_scale),
                Visibility::default(),
                PREVIEW_RENDER_LAYER,
            ));
        });
    }
}
