use bevy::prelude::*;

use crate::units::input::ray_sphere_hit_distance;
use crate::world::{BuildingCatalog, BuildingId, BuildingRecord, WorldData};

use super::components::BuildingRenderEntity;
use super::placeholder::building_pick_radius;

/// Pick the nearest building along a screen ray using render entity transforms.
pub fn pick_building_along_ray(
    ray: &Ray3d,
    world: &WorldData,
    catalog: &BuildingCatalog,
    buildings: &Query<(&BuildingRenderEntity, &GlobalTransform)>,
) -> Option<BuildingId> {
    pick_building_along_ray_with_distance(ray, world, catalog, buildings).map(|(_, id)| id)
}

/// Like [`pick_building_along_ray`] but returns ray distance for click precedence.
pub fn pick_building_along_ray_with_distance(
    ray: &Ray3d,
    world: &WorldData,
    catalog: &BuildingCatalog,
    buildings: &Query<(&BuildingRenderEntity, &GlobalTransform)>,
) -> Option<(f32, BuildingId)> {
    let mut best: Option<(f32, BuildingId)> = None;

    for (marker, transform) in buildings {
        let Some(record) = world.get_building(marker.building_id) else {
            continue;
        };
        let Some(definition) = catalog.get(&record.definition_id) else {
            continue;
        };
        let radius = building_pick_radius(definition, record.placement.uniform_scale_f32());
        let center = transform.translation() + Vec3::Y * (radius * 0.35);
        let Some(distance) = ray_sphere_hit_distance(ray, center, radius) else {
            continue;
        };
        if best.map(|(best_t, _)| distance < best_t).unwrap_or(true) {
            best = Some((distance, marker.building_id));
        }
    }

    best
}
