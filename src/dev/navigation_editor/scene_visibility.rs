//! Per-instance building opacity during Navigation Editor sessions.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::alpha::AlphaMode;

use crate::buildings::BuildingRenderIndex;
use crate::dev::inspector::BlueprintInspectionState;
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::world::BuildingId;

use super::state::{NavigationEditorUiState, navigation_editor_owns_session};

#[derive(Resource, Default)]
pub struct BlueprintInspectionScenePresentation {
    tracked_building: Option<BuildingId>,
    saved_materials: HashMap<Entity, Handle<StandardMaterial>>,
    /// Temporary faded variants owned by the editor; released on restore/reapply.
    faded_materials: HashMap<Entity, Handle<StandardMaterial>>,
    applied_opacity: f32,
}

pub fn sync_blueprint_inspection_scene_visibility(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<DevWindowRegistry>,
    inspection: Res<BlueprintInspectionState>,
    ui_state: Res<NavigationEditorUiState>,
    render_index: Res<BuildingRenderIndex>,
    mut presentation: ResMut<BlueprintInspectionScenePresentation>,
    children: Query<&Children>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mesh_materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    let session_active = navigation_editor_owns_session(dev_state.enabled, &registry, &inspection);

    if !session_active {
        restore_presentation(&mut presentation, &mut materials, &mut mesh_materials);
        return;
    }

    let Some(building_id) = inspection.building_id else {
        restore_presentation(&mut presentation, &mut materials, &mut mesh_materials);
        return;
    };

    if presentation.tracked_building != Some(building_id) {
        restore_presentation(&mut presentation, &mut materials, &mut mesh_materials);
        presentation.tracked_building = Some(building_id);
        presentation.applied_opacity = f32::NAN;
    }

    let opacity = ui_state.building_opacity.clamp(0.0, 1.0);
    if (presentation.applied_opacity - opacity).abs() < 0.001 {
        return;
    }

    let Some(root) = render_index.0.get(&building_id).copied() else {
        return;
    };

    let mut descendants = Vec::new();
    collect_descendants(root, &children, &mut descendants);

    for entity in descendants {
        apply_entity_opacity(
            entity,
            opacity,
            &mut presentation,
            &mut materials,
            &mut mesh_materials,
        );
    }

    presentation.applied_opacity = opacity;
}

fn collect_descendants(entity: Entity, children: &Query<&Children>, out: &mut Vec<Entity>) {
    out.push(entity);
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            collect_descendants(child, children, out);
        }
    }
}

fn apply_entity_opacity(
    entity: Entity,
    opacity: f32,
    presentation: &mut BlueprintInspectionScenePresentation,
    materials: &mut Assets<StandardMaterial>,
    mesh_materials: &mut Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    let Ok(material_component) = mesh_materials.get(entity) else {
        return;
    };
    let current = material_component.0.clone();

    let original = presentation
        .saved_materials
        .entry(entity)
        .or_insert(current)
        .clone();

    if let Some(stale) = presentation.faded_materials.remove(&entity) {
        materials.remove(&stale);
    }

    let restored = if opacity >= 0.999 {
        original.clone()
    } else {
        let Some(source) = materials.get(&original) else {
            return;
        };
        let mut faded = source.clone();
        faded.base_color.set_alpha(opacity);
        faded.alpha_mode = AlphaMode::Blend;
        let handle = materials.add(faded);
        presentation.faded_materials.insert(entity, handle.clone());
        handle
    };

    let Ok(mut material_component) = mesh_materials.get_mut(entity) else {
        return;
    };
    material_component.0 = restored;
}

fn restore_presentation(
    presentation: &mut BlueprintInspectionScenePresentation,
    materials: &mut Assets<StandardMaterial>,
    mesh_materials: &mut Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    for (entity, original) in presentation.saved_materials.drain() {
        if let Ok(mut material) = mesh_materials.get_mut(entity) {
            material.0 = original;
        }
    }
    for (_entity, faded) in presentation.faded_materials.drain() {
        materials.remove(&faded);
    }
    presentation.tracked_building = None;
    presentation.applied_opacity = f32::NAN;
}
