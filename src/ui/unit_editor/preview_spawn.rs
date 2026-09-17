//! Spawn and refresh the isolated preview unit scene (CG3).

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::camera::render_layers::PREVIEW_RENDER_LAYER;
use crate::units::{
    UnitAppearanceMorphFingerprint, UnitRenderMetadata, UnitSceneAssets, UnitSceneRoot,
};
use crate::units::presentation::{
    UnitEditorPreviewFraming, UnitEditorPreviewRoot, UnitEditorPreviewUnit,
    UnitPresentationAppearance,
};
use crate::world::{
    AppearanceProfileCatalog, UnitAppearance, UnitCatalog, effective_render_key_for_appearance,
    unit_visual_scale,
};

use super::session::UnitEditorSession;

/// Whether a preview GLB must be respawned (render identity) vs updated in place.
pub fn appearance_requires_preview_respawn(current: &UnitAppearance, next: &UnitAppearance) -> bool {
    current.profile_id != next.profile_id || current.body_variant_id != next.body_variant_id
}

/// Spawn or refresh the preview unit from the active editor draft.
pub fn sync_unit_editor_preview(
    mut commands: Commands,
    session: Option<Res<UnitEditorSession>>,
    catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<UnitSceneAssets>,
    preview_roots: Query<Entity, With<UnitEditorPreviewRoot>>,
    preview_units: Query<
        (Entity, &UnitPresentationAppearance, &Transform),
        With<UnitEditorPreviewUnit>,
    >,
) {
    let Some(session) = session else {
        return;
    };
    let Some(definition) = catalog.get(&session.draft.definition_id) else {
        return;
    };
    let render_key = match effective_render_key_for_appearance(
        &session.draft.appearance,
        &appearance_profiles,
    ) {
        Ok(key) => key,
        Err(error) => {
            warn!("unit editor preview render key failed: {error}");
            return;
        }
    };
    let render_key_str = render_key.0.as_deref().unwrap_or("");
    let Some(scene) = scene_assets.scene_for_render_key(render_key_str).cloned() else {
        if !render_key_str.is_empty() {
            scene_assets.log_missing_once(render_key_str);
        }
        return;
    };
    if !scene_is_loaded(&asset_server, &scene) {
        return;
    }

    let visual_scale = unit_visual_scale(definition, session.draft.appearance.height_scale);
    let appearance = session.draft.appearance.clone();
    let definition_id = session.draft.definition_id.clone();

    let Some(parent) = preview_roots.iter().next() else {
        return;
    };

    if let Some((entity, current, transform)) = preview_units.iter().next() {
        if current.appearance == appearance {
            return;
        }
        if appearance_requires_preview_respawn(&current.appearance, &appearance) {
            commands.entity(entity).despawn();
        } else {
            commands.entity(entity).insert((
                UnitPresentationAppearance { appearance },
                Transform {
                    translation: transform.translation,
                    rotation: transform.rotation,
                    scale: visual_scale,
                },
            ));
            commands
                .entity(entity)
                .remove::<UnitAppearanceMorphFingerprint>();
            commands.entity(entity).remove::<UnitEditorPreviewFraming>();
            return;
        }
    }

    commands.entity(parent).with_children(|parent| {
        parent.spawn((
            UnitEditorPreviewUnit,
            UnitPresentationAppearance { appearance },
            UnitRenderMetadata { definition_id },
            UnitSceneRoot,
            SceneRoot(scene),
            Transform::from_scale(visual_scale),
            Visibility::default(),
            PREVIEW_RENDER_LAYER,
        ));
    });
}

fn scene_is_loaded(asset_server: &AssetServer, scene: &Handle<Scene>) -> bool {
    matches!(asset_server.get_load_state(scene), Some(LoadState::Loaded))
}
