//! Gameplay left-click target resolution (units vs buildings).

use bevy::prelude::*;

use crate::buildings::components::BuildingRenderEntity;
use crate::buildings::picking::pick_building_along_ray_with_distance;
use crate::units::UnitRenderEntity;
use crate::units::input::pick_unit_along_ray_with_distance;
use crate::world::{
    BuildingCatalog, BuildingId, SelectionControllabilityPolicy, UnitCatalog, UnitId, WorldData,
};

/// Left-click world object resolved along a screen ray.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickPickTarget {
    Unit(UnitId),
    Building(BuildingId),
}

/// Pick the nearest unit or building along `ray` using existing render-entity hit tests.
pub fn pick_click_target_along_ray(
    ray: &Ray3d,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    building_catalog: &BuildingCatalog,
    units: &Query<(&UnitRenderEntity, &GlobalTransform)>,
    buildings: &Query<(&BuildingRenderEntity, &GlobalTransform)>,
    selection_policy: SelectionControllabilityPolicy,
) -> Option<ClickPickTarget> {
    let unit = pick_unit_along_ray_with_distance(ray, world, unit_catalog, units, selection_policy);
    let building = pick_building_along_ray_with_distance(ray, world, building_catalog, buildings);

    match (unit, building) {
        (Some((unit_distance, unit_id)), Some((building_distance, building_id))) => {
            if unit_distance <= building_distance {
                Some(ClickPickTarget::Unit(unit_id))
            } else {
                Some(ClickPickTarget::Building(building_id))
            }
        }
        (Some((_, unit_id)), None) => Some(ClickPickTarget::Unit(unit_id)),
        (None, Some((_, building_id))) => Some(ClickPickTarget::Building(building_id)),
        (None, None) => None,
    }
}
