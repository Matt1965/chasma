//! Per-instance GLB material preparation (ADR-095 BA1).
//!
//! Clones scene materials before styling so ghost/lifecycle passes never mutate shared
//! glTF asset materials (which would break other instances and dev reloads).

use bevy::prelude::*;

use crate::world::{Affiliation, BuildingLifecycleState};

use super::placeholder::lifecycle_building_color;

/// Clone and style mesh materials for one scene hierarchy.
pub fn prepare_scene_materials(
    commands: &mut Commands,
    entity: Entity,
    children: &Query<&Children>,
    mesh_materials: &Query<&MeshMaterial3d<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
    lifecycle: BuildingLifecycleState,
    affiliation: Affiliation,
) -> bool {
    let mut touched = false;
    if let Ok(mesh_material) = mesh_materials.get(entity) {
        let mut cloned = materials
            .get(&mesh_material.0)
            .cloned()
            .unwrap_or_default();
        apply_lifecycle_material_style(&mut cloned, lifecycle, affiliation);
        let handle = materials.add(cloned);
        commands.entity(entity).insert(MeshMaterial3d(handle));
        touched = true;
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            touched |= prepare_scene_materials(
                commands,
                child,
                children,
                mesh_materials,
                materials,
                lifecycle,
                affiliation,
            );
        }
    }
    touched
}

fn apply_lifecycle_material_style(
    material: &mut StandardMaterial,
    lifecycle: BuildingLifecycleState,
    affiliation: Affiliation,
) {
    material.unlit = true;
    match lifecycle {
        BuildingLifecycleState::Complete => {
            material.alpha_mode = AlphaMode::Opaque;
        }
        _ => {
            let color = lifecycle_building_color(lifecycle, affiliation);
            material.base_color = color;
            material.alpha_mode = if color.to_srgba().alpha < 0.99 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            };
        }
    }
}
