//! Presentation-only transform preview override (ADR-099).

use bevy::prelude::*;

/// Marks a render entity displaying a client-local dev transform preview.
/// Removed on commit, cancel, or selection change. Not authoritative.
#[derive(Component, Debug, Clone, Copy)]
pub struct DevTransformPreview {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

/// Apply preview transforms to doodad render entities after authoritative sync.
pub fn apply_doodad_transform_preview(
    edit: Res<super::state::TransformEditState>,
    render_index: Res<crate::doodads::DoodadRenderIndex>,
    catalog: Res<crate::world::DoodadCatalog>,
    config: Res<crate::world::WorldConfig>,
    world: Res<crate::world::WorldData>,
    render_assets: Option<Res<crate::terrain::TerrainRenderAssets>>,
    mut transforms: Query<&mut Transform>,
    mut commands: Commands,
    preview: Query<Entity, With<DevTransformPreview>>,
) {
    for entity in &preview {
        commands.entity(entity).remove::<DevTransformPreview>();
    }

    let Some(target) = edit.target else {
        return;
    };
    let super::tool::SelectedWorldObject::Doodad(doodad_id) = target else {
        return;
    };
    let Some(preview_placement) = edit.preview_placement else {
        return;
    };
    let Some(entity) = render_index.0.get(&doodad_id).copied() else {
        return;
    };
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let layout = config.chunk_layout();
    let translation = crate::terrain::world_position_to_render_global(
        preview_placement.position,
        layout,
        vertical_scale,
    );
    let rotation = preview_placement.rotation_quat();
    let scale = world
        .get_doodad(doodad_id)
        .and_then(|record| catalog.get(&record.definition_id))
        .map(|definition| {
            crate::world::doodad_final_render_scale(definition, preview_placement.scale_vec3())
        })
        .unwrap_or_else(|| preview_placement.scale_vec3());

    if let Ok(mut transform) = transforms.get_mut(entity) {
        transform.translation = translation;
        transform.rotation = rotation;
        transform.scale = scale;
    }
    commands.entity(entity).insert(DevTransformPreview {
        translation,
        rotation,
        scale,
    });
}

/// Apply the client-local transform preview to the selected building's render entity.
///
/// Buildings render as either a single scene entity or an anchor entity with a model
/// child (offset/baseline scaling). This mirrors `sync_building_render_entities` but
/// sources the placement from `edit.preview_placement`, so translate/rotate/scale are
/// visible live during a drag instead of only after commit. It must run after building
/// sync (guaranteed: `DevModePresentationSystems` is chained after `RuntimeSyncSystems`).
pub fn apply_building_transform_preview(
    edit: Res<super::state::TransformEditState>,
    render_index: Res<crate::buildings::BuildingRenderIndex>,
    catalog: Res<crate::world::BuildingCatalog>,
    config: Res<crate::world::WorldConfig>,
    world: Res<crate::world::WorldData>,
    render_assets: Option<Res<crate::terrain::TerrainRenderAssets>>,
    markers: Query<&crate::buildings::BuildingRenderEntity>,
    children_q: Query<&Children>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(super::tool::SelectedWorldObject::Building(building_id)) = edit.target else {
        return;
    };
    if !edit.mode.is_transform() {
        return;
    }
    let Some(preview) = edit.preview_placement else {
        return;
    };
    let Some(record) = world.get_building(building_id) else {
        return;
    };
    let Some(definition) = catalog.get(&record.definition_id) else {
        return;
    };
    let Some(entity) = render_index.0.get(&building_id).copied() else {
        return;
    };

    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let layout = config.chunk_layout();
    let uniform_scale = super::state::building_uniform_scale_from_preview(preview);
    let placement =
        crate::world::BuildingPlacement::new(preview.position, preview.orientation.to_quat())
            .with_uniform_scale(uniform_scale);

    let uses_fallback = markers
        .get(entity)
        .map(|m| m.uses_diagnostic_fallback)
        .unwrap_or(false);
    if uses_fallback {
        // Diagnostic cuboid: ground anchor + half-height bump, yaw only, no model scaling.
        let mut translation = crate::terrain::world_position_to_render_global(
            placement.position,
            layout,
            vertical_scale,
        );
        let mesh_size = crate::buildings::placeholder::placeholder_mesh_size(definition);
        translation.y += mesh_size.y * 0.5;
        if let Ok(mut transform) = transforms.get_mut(entity) {
            transform.translation = translation;
            transform.rotation = placement.rotation;
            transform.scale = Vec3::ONE;
        }
        return;
    }

    if crate::world::building_uses_model_child(definition) {
        let anchor = crate::world::building_anchor_render_transform(
            definition,
            &placement,
            layout,
            vertical_scale,
        );
        if let Ok(mut transform) = transforms.get_mut(entity) {
            *transform = anchor;
        }
        // Uniform scale lives on the model child, so update it for live scale preview.
        let correction = crate::world::building_model_child_local_transform(
            definition,
            placement.uniform_scale_f32(),
        );
        if let Ok(children) = children_q.get(entity) {
            for child in children.iter() {
                if let Ok(mut child_transform) = transforms.get_mut(child) {
                    *child_transform = correction;
                }
            }
        }
    } else {
        let transform = crate::world::building_model_render_transform(
            definition,
            &placement,
            layout,
            vertical_scale,
        );
        if let Ok(mut existing) = transforms.get_mut(entity) {
            *existing = transform;
        }
    }
}
