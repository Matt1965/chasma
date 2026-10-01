//! Head-and-shoulders camera framing for portrait capture.

use bevy::math::{Affine3A, Vec3A};
use bevy::prelude::*;

use crate::world::{UnitDefinition, UnitDefinitionId};

use super::components::UnitPortraitFraming;

pub const PORTRAIT_FOCUS_FALLBACK_Y: f32 = 1.45;
pub const PORTRAIT_HEAD_FALLBACK_HEIGHT: f32 = 0.35;
pub const PORTRAIT_DISTANCE_BASE: f32 = 0.95;
pub const PORTRAIT_UNIT_YAW: f32 = std::f32::consts::PI;

/// Optional per-definition portrait tuning (data-driven overrides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PortraitFramingOverride {
    pub focus_height_fraction: f32,
    pub distance_scale: f32,
}

impl Default for PortraitFramingOverride {
    fn default() -> Self {
        Self {
            focus_height_fraction: 0.82,
            distance_scale: 1.0,
        }
    }
}

/// Known tuning for unusually proportioned rigs (extend as needed).
pub fn portrait_framing_override(definition_id: &UnitDefinitionId) -> PortraitFramingOverride {
    match definition_id.as_str() {
        "giant_slug" | "telluropod" => PortraitFramingOverride {
            focus_height_fraction: 0.65,
            distance_scale: 1.15,
        },
        _ => PortraitFramingOverride::default(),
    }
}

/// Whole-model framing for diagnostic step 5.
pub fn portrait_generous_framing_from_bounds(center: Vec3, body_height: f32) -> UnitPortraitFraming {
    let height = body_height.max(0.8);
    UnitPortraitFraming {
        focus: Vec3::new(center.x, center.y, center.z),
        head_height: height * 1.15,
    }
}

pub fn portrait_framing_from_bounds(
    center: Vec3,
    body_height: f32,
    definition: &UnitDefinition,
) -> UnitPortraitFraming {
    let tuning = portrait_framing_override(&definition.id);
    let head_height = (body_height * 0.42).clamp(0.22, 1.2);
    let focus_y = center.y + body_height * tuning.focus_height_fraction - body_height * 0.5;
    UnitPortraitFraming {
        focus: Vec3::new(center.x, focus_y, center.z),
        head_height: head_height * tuning.distance_scale,
    }
}

pub fn portrait_camera_transform(framing: &UnitPortraitFraming) -> Transform {
    let distance = PORTRAIT_DISTANCE_BASE
        * (framing.head_height / PORTRAIT_HEAD_FALLBACK_HEIGHT).clamp(0.7, 1.45);
    let offset = Vec3::new(0.0, framing.head_height * 0.05, distance);
    Transform::from_translation(framing.focus + offset).looking_at(framing.focus, Vec3::Y)
}

pub fn measure_actor_bounds(
    root: Entity,
    children: &Query<&Children>,
    mesh3d: &Query<&Mesh3d>,
    meshes: &Assets<Mesh>,
    global_transforms: &Query<&GlobalTransform>,
) -> Option<(Vec3, f32)> {
    let mut min = Vec3A::splat(f32::INFINITY);
    let mut max = Vec3A::splat(f32::NEG_INFINITY);
    let mut found = false;

    for entity in descendants(root, children) {
        let Ok(mesh3d) = mesh3d.get(entity) else {
            continue;
        };
        let Some(mesh) = meshes.get(&mesh3d.0) else {
            continue;
        };
        let Some(aabb) = mesh.final_aabb else {
            continue;
        };
        let Ok(global) = global_transforms.get(entity) else {
            continue;
        };
        let matrix = Affine3A::from(global.affine());
        let corners = [
            Vec3A::from(aabb.min),
            Vec3A::from(aabb.max),
            Vec3A::new(aabb.min.x, aabb.min.y, aabb.max.z),
            Vec3A::new(aabb.min.x, aabb.max.y, aabb.min.z),
            Vec3A::new(aabb.max.x, aabb.min.y, aabb.min.z),
            Vec3A::new(aabb.min.x, aabb.max.y, aabb.max.z),
            Vec3A::new(aabb.max.x, aabb.min.y, aabb.max.z),
            Vec3A::new(aabb.max.x, aabb.max.y, aabb.min.z),
        ];
        for corner in corners {
            let world = matrix.transform_point3a(corner);
            min = min.min(world);
            max = max.max(world);
            found = true;
        }
    }

    if !found {
        return None;
    }
    let center = Vec3::from((min + max) * 0.5);
    let height = (max.y - min.y).max(0.5);
    Some((center, height))
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
