//! Render-entity unit picking and screen projection (ADR-034 U9).

use bevy::camera::Camera;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::units::UnitRenderEntity;
use crate::world::{unit_is_selectable, SelectionControllabilityPolicy, UnitCatalog, UnitDefinition, UnitId, WorldData};

/// Minimum pick radius so small unit meshes remain clickable (meters).
const MIN_UNIT_PICK_RADIUS_METERS: f32 = 1.5;

/// Scale catalog collision radius into a generous screen-pick volume.
const UNIT_PICK_RADIUS_SCALE: f32 = 2.5;

/// Build a world-space ray from the primary window cursor through the RTS camera.
pub fn cursor_world_ray(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
) -> Option<Ray3d> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, camera_transform) = camera.single().ok()?;
    camera.viewport_to_world(camera_transform, cursor).ok()
}

/// Current cursor position in window space.
pub fn cursor_screen_position(windows: &Query<&Window, With<PrimaryWindow>>) -> Option<Vec2> {
    windows.single().ok()?.cursor_position()
}

/// Project a render-space world position to window coordinates.
pub fn world_position_to_screen(
    render_global: Vec3,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec2> {
    camera.world_to_viewport(camera_transform, render_global).ok()
}

/// Pick the front-most visible unit render entity intersecting `ray`.
pub fn pick_unit_along_ray(
    ray: &Ray3d,
    world: &WorldData,
    catalog: &UnitCatalog,
    units: &Query<(&UnitRenderEntity, &GlobalTransform)>,
    policy: SelectionControllabilityPolicy,
) -> Option<UnitId> {
    let mut best: Option<(f32, UnitId)> = None;

    for (marker, transform) in units {
        let Some(record) = world.get_unit(marker.unit_id) else {
            continue;
        };
        let Some(definition) = catalog.get(&record.definition_id) else {
            continue;
        };
        if !unit_is_selectable(record, policy) {
            continue;
        }
        let radius = unit_pick_radius(definition);
        let center = transform.translation() + Vec3::Y * (radius * 0.35);
        let Some(distance) = ray_sphere_hit_distance(ray, center, radius) else {
            continue;
        };
        if best.map(|(best_t, _)| distance < best_t).unwrap_or(true) {
            best = Some((distance, marker.unit_id));
        }
    }

    best.map(|(_, id)| id)
}

pub fn unit_pick_radius(definition: &UnitDefinition) -> f32 {
    (definition.collision_radius_meters * UNIT_PICK_RADIUS_SCALE).max(MIN_UNIT_PICK_RADIUS_METERS)
}

pub fn ray_sphere_hit_distance(ray: &Ray3d, center: Vec3, radius: f32) -> Option<f32> {
    let direction = ray.direction.as_vec3();
    let origin_to_center = ray.origin - center;
    let a = direction.dot(direction);
    let b = 2.0 * origin_to_center.dot(direction);
    let c = origin_to_center.dot(origin_to_center) - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }

    let sqrt = discriminant.sqrt();
    let inv = 1.0 / (2.0 * a);
    let t0 = (-b - sqrt) * inv;
    let t1 = (-b + sqrt) * inv;
    [t0, t1]
        .into_iter()
        .filter(|t| *t > 0.0)
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{UnitDefinition, UnitDefinitionId, UnitRenderKey};

    #[test]
    fn ray_sphere_returns_nearest_positive_hit() {
        let ray = Ray3d {
            origin: Vec3::new(0.0, 5.0, 0.0),
            direction: Dir3::new(Vec3::NEG_Y).unwrap(),
        };
        let hit = ray_sphere_hit_distance(&ray, Vec3::new(0.0, 0.0, 0.0), 1.0).unwrap();
        assert!((hit - 4.0).abs() < 1e-4);
    }

    #[test]
    fn unit_pick_radius_has_a_usability_floor() {
        let definition = UnitDefinition::new(
            UnitDefinitionId::new("tiny"),
            "Tiny",
            "Test",
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1.0,
            "Common",
            4.0,
            0.1,
            40.0,
            crate::world::WeaponDefinitionId::new("weapon_fists"),
            true,
            UnitRenderKey::unset(),
        );
        assert_eq!(unit_pick_radius(&definition), MIN_UNIT_PICK_RADIUS_METERS);
    }
}
