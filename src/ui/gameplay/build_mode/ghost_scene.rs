//! Build mode GLB ghost preview (ADR-081 B4, ADR-095 BA1).

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::buildings::assets::{BuildingSceneAssets, ghost_render_key};
use crate::buildings::scene_materials::prepare_scene_materials;
use crate::terrain::TerrainRenderAssets;
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState, BuildingPlacement,
    WorldConfig, building_anchor_render_transform, building_has_model_correction,
    building_model_correction_local_transform, building_model_render_transform,
    rotation_from_quadrants,
};

use super::preview::BuildModeCursorAnchor;
use super::state::BuildModeState;

/// Client-local ghost scene entity (never authoritative).
#[derive(Component, Debug)]
pub struct BuildModeGhostScene {
    pub definition_id: BuildingDefinitionId,
    pub render_key: String,
}

#[derive(Component, Debug, Default)]
pub struct BuildModeGhostTintPending;

/// Sync the translucent building model ghost while placement mode is active.
pub fn sync_build_mode_ghost_scene(
    mut commands: Commands,
    build_mode: Res<BuildModeState>,
    anchor: Res<BuildModeCursorAnchor>,
    building_catalog: Res<BuildingCatalog>,
    config: Res<WorldConfig>,
    asset_server: Res<AssetServer>,
    mut scene_assets: ResMut<BuildingSceneAssets>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut existing: Query<(Entity, &BuildModeGhostScene)>,
) {
    let has_anchor = build_mode
        .last_plan
        .as_ref()
        .is_some()
        || anchor.position.is_some();
    let should_show =
        build_mode.is_ghost_placing() && build_mode.ghost_definition_id().is_some() && has_anchor;

    if !should_show {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    }

    let definition_id = build_mode.ghost_definition_id().unwrap().clone();
    let Some(definition) = building_catalog.get(&definition_id) else {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    };
    let Some(render_key) = ghost_render_key(definition) else {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    };
    let Some(scene) = scene_assets.ensure_scene(&render_key, &asset_server) else {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    };
    if !matches!(asset_server.get_load_state(&scene), Some(LoadState::Loaded)) {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    }

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let anchor_position = build_mode
        .last_plan
        .as_ref()
        .map(|plan| plan.grounded_anchor)
        .or(anchor.position);
    let Some(anchor_position) = anchor_position else {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    };
    let placement = BuildingPlacement::new(
        anchor_position,
        rotation_from_quadrants(build_mode.ghost_rotation_quadrants()),
    );

    if let Some((entity, marker)) = existing.iter().next() {
        if marker.definition_id != definition_id || marker.render_key != render_key {
            commands.entity(entity).despawn();
        } else {
            if building_has_model_correction(definition) {
                let anchor_transform = building_anchor_render_transform(
                    definition,
                    &placement,
                    layout,
                    vertical_scale,
                );
                commands.entity(entity).insert(anchor_transform);
            } else {
                let transform = building_model_render_transform(
                    definition,
                    &placement,
                    layout,
                    vertical_scale,
                );
                commands.entity(entity).insert(transform);
            }
            commands.entity(entity).insert(BuildModeGhostTintPending);
            return;
        }
    }

    let ghost = (
        BuildModeGhostScene {
            definition_id,
            render_key,
        },
        BuildModeGhostTintPending,
        Visibility::default(),
    );

    if building_has_model_correction(definition) {
        let anchor_transform =
            building_anchor_render_transform(definition, &placement, layout, vertical_scale);
        let correction = building_model_correction_local_transform(definition);
        commands
            .spawn((ghost, anchor_transform))
            .with_children(|parent| {
                parent.spawn((SceneRoot(scene), correction));
            });
    } else {
        let transform =
            building_model_render_transform(definition, &placement, layout, vertical_scale);
        commands.spawn((ghost, SceneRoot(scene), transform));
    }
}

/// Apply planned-state tint to ghost scene descendants after spawn.
pub fn tint_build_mode_ghost_scene(
    mut commands: Commands,
    ghosts: Query<(Entity, &BuildModeGhostScene), With<BuildModeGhostTintPending>>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, _) in &ghosts {
        if !prepare_scene_materials(
            &mut commands,
            entity,
            &children,
            &mesh_materials,
            &mut materials,
            BuildingLifecycleState::Planned,
            Affiliation::Player,
        ) {
            continue;
        }
        commands
            .entity(entity)
            .remove::<BuildModeGhostTintPending>();
    }
}
