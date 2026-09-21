//! Isolated Unit Editor preview markers and render-layer propagation (CG3).

use bevy::prelude::*;

use crate::camera::render_layers::PREVIEW_RENDER_LAYER;

/// Root entity for preview studio resources (camera, lights, stage environment).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitEditorPreviewRoot;

/// Stage-owned environment meshes (ground, backdrop). Not gameplay terrain.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitEditorPreviewEnvironment;

/// Preview-stage ground surface beneath roster actors.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitEditorPreviewGround;

/// Parent for future stage dressing (props, fog, biome decor).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitEditorPreviewDressingRoot;

/// Marker on the preview unit scene root — not a gameplay [`UnitRenderEntity`].
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitEditorPreviewUnit;

/// World-space body framing for preview camera orbit (CG3.1).
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct UnitEditorPreviewFraming {
    pub body_center: Vec3,
    pub body_height: f32,
}

/// Apply render-layer isolation to all preview-scene descendants.
pub fn propagate_preview_render_layers(
    mut commands: Commands,
    preview_roots: Query<Entity, With<UnitEditorPreviewRoot>>,
    children: Query<&Children>,
    layers: Query<&bevy::camera::visibility::RenderLayers>,
) {
    for root in &preview_roots {
        for entity in descendants(root, &children) {
            if layers.get(entity).is_ok() {
                continue;
            }
            commands.entity(entity).insert(PREVIEW_RENDER_LAYER);
        }
    }
}

fn descendants(root: Entity, children: &Query<&Children>) -> Vec<Entity> {
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(entity) = stack.pop() {
        out.push(entity);
        if let Ok(kids) = children.get(entity) {
            for child in kids.iter() {
                stack.push(child);
            }
        }
    }
    out
}
