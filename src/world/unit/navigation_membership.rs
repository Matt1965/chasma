//! Explicit spawn/load navigation membership initialization (IN-11gG-M).
//!
//! Tracked [`SpaceId`] is simulation truth. Positional inference establishes that truth
//! only at explicit lifecycle boundaries — not during ordinary movement ticks.

use super::id::UnitId;
use crate::world::{
    SpaceId, WorldData, WorldPosition, interior_navigation_move_target_at_position,
};

/// Infer navigable membership from hydrated runtime geometry at an explicit boundary.
///
/// Reuses [`interior_navigation_move_target_at_position`] (floor tolerance + walkable region)
/// rather than inventing a separate containment implementation.
pub fn infer_navigation_membership_at_position(
    world: &WorldData,
    position: WorldPosition,
) -> SpaceId {
    interior_navigation_move_target_at_position(
        world.building_navigation_runtime(),
        world.space_registry(),
        world.layout(),
        position,
    )
    .unwrap_or(SpaceId::SURFACE)
}

/// Initialize tracked membership for one unit from its current placement.
pub fn initialize_unit_navigation_membership(world: &mut WorldData, unit_id: UnitId) -> bool {
    let (position, tracked) = match world.get_unit(unit_id) {
        Some(record) => (record.placement.position, record.current_space_id),
        None => return false,
    };
    let inferred = infer_navigation_membership_at_position(world, position);
    if inferred != tracked {
        if world.set_unit_current_space(unit_id, inferred).is_err() {
            return false;
        }
    }
    if inferred.is_surface() {
        return true;
    }
    let grounded =
        crate::world::ground_position_in_space(world, world.space_registry(), inferred, position);
    if let Some(grounded) = grounded {
        let current = world
            .get_unit(unit_id)
            .map(|record| record.placement.position);
        if current.is_some_and(|current| current != grounded) {
            return world.update_unit_position(unit_id, grounded).is_ok();
        }
    }
    true
}

/// Upgrade Surface-tracked units when geometry says they occupy an interior region.
///
/// Preserves persisted or transitioned non-Surface membership.
pub fn initialize_unit_navigation_membership_if_surface(
    world: &mut WorldData,
    unit_id: UnitId,
) -> bool {
    let tracked = match world.get_unit(unit_id) {
        Some(record) => record.current_space_id,
        None => return false,
    };
    if !tracked.is_surface() {
        return true;
    }
    initialize_unit_navigation_membership(world, unit_id)
}

/// Post-hydration pass: infer membership for units still tracked on Surface.
pub fn initialize_surface_units_navigation_membership(world: &mut WorldData) {
    let unit_ids = world.sorted_unit_ids();
    for unit_id in unit_ids {
        initialize_unit_navigation_membership_if_surface(world, unit_id);
    }
}
