//! Derived Ground/Water locomotion and support height.

use super::state::WorldWaterState;
use crate::world::unit::{CombatState, UnitId, UnitOrder, UnitState, unit_can_execute_actions};
use crate::world::{
    SpaceId, UnitCatalog, WeaponCatalog, WorldData, WorldPosition, cancel_unit_task,
    clear_attack_cycle_for_order_cancel, ground_world_position,
};
use crate::world::task::TaskCancelReason;

/// Environmental locomotion surface. Not a `UnitState` and not persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LocomotionSurface {
    #[default]
    Ground,
    Water,
}

impl LocomotionSurface {
    pub fn is_water(self) -> bool {
        matches!(self, Self::Water)
    }
}

/// Depth at `position` when water is enabled and terrain is sampleable.
pub fn water_depth_at(world: &WorldData, position: WorldPosition) -> Option<f32> {
    let water = world.water();
    if !water.enabled {
        return None;
    }
    let terrain_y = ground_world_position(world, position)?.local.0.y;
    Some(water.surface_y_sim - terrain_y)
}

/// Resolve Ground vs Water from depth and previous surface (hysteresis).
pub fn locomotion_surface_at(
    water: &WorldWaterState,
    depth: Option<f32>,
    previous: Option<LocomotionSurface>,
) -> LocomotionSurface {
    if !water.enabled {
        return LocomotionSurface::Ground;
    }
    let Some(depth) = depth else {
        return LocomotionSurface::Ground;
    };
    let threshold = match previous {
        Some(LocomotionSurface::Water) => water.exit_depth_sim,
        Some(LocomotionSurface::Ground) | None => water.enter_depth_sim,
    };
    if depth >= threshold {
        LocomotionSurface::Water
    } else {
        LocomotionSurface::Ground
    }
}

/// Support Y for a space + optional previous surface (pathfinding uses `previous = None`).
pub fn sample_locomotion_support_height(
    world: &WorldData,
    space_id: SpaceId,
    position: WorldPosition,
    previous: Option<LocomotionSurface>,
) -> Option<(f32, LocomotionSurface)> {
    if !space_id.is_surface() {
        let space = world.space_registry().get_space(space_id)?;
        if !space.enabled || !space.walkable {
            return None;
        }
        return Some((space.floor_y_global, LocomotionSurface::Ground));
    }
    let terrain = ground_world_position(world, position)?;
    let terrain_y = terrain.local.0.y;
    let depth = if world.water().enabled {
        Some(world.water().surface_y_sim - terrain_y)
    } else {
        None
    };
    let surface = locomotion_surface_at(world.water(), depth, previous);
    let y = match surface {
        LocomotionSurface::Ground => terrain_y,
        LocomotionSurface::Water => world.water().surface_y_sim + world.water().origin_offset_sim,
    };
    Some((y, surface))
}

pub fn water_skips_seabed_slope(world: &WorldData, position: WorldPosition) -> bool {
    let depth = water_depth_at(world, position);
    locomotion_surface_at(world.water(), depth, None).is_water()
}

pub fn unit_locomotion_surface(world: &WorldData, unit_id: UnitId) -> LocomotionSurface {
    if let Some(stored) = world.locomotion_surface(unit_id) {
        return stored;
    }
    let Some(record) = world.get_unit(unit_id) else {
        return LocomotionSurface::Ground;
    };
    sample_locomotion_support_height(
        world,
        record.current_space_id,
        record.placement.position,
        None,
    )
    .map(|(_, surface)| surface)
    .unwrap_or(LocomotionSurface::Ground)
}

pub fn unit_is_swimming(world: &WorldData, unit_id: UnitId) -> bool {
    unit_locomotion_surface(world, unit_id).is_water()
}

pub fn unit_can_perform_normal_actions(world: &WorldData, unit_id: UnitId) -> bool {
    unit_can_execute_actions(world, unit_id) && !unit_is_swimming(world, unit_id)
}

pub fn unit_order_requires_normal_actions(order: UnitOrder) -> bool {
    !matches!(order, UnitOrder::Idle | UnitOrder::MoveTo { .. })
}

pub fn effective_move_speed_mps(
    catalog_speed_mps: f32,
    surface: LocomotionSurface,
    water: &WorldWaterState,
) -> f32 {
    match surface {
        LocomotionSurface::Ground => catalog_speed_mps,
        LocomotionSurface::Water => catalog_speed_mps * water.speed_multiplier,
    }
}

/// Recompute surfaces, snap idle support Y, and cancel incompatible actions on enter.
pub fn refresh_all_unit_locomotion(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) {
    let unit_ids = world.sorted_unit_ids();
    for unit_id in unit_ids {
        refresh_unit_locomotion(world, unit_catalog, weapon_catalog, unit_id);
    }
}

fn refresh_unit_locomotion(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    unit_id: UnitId,
) {
    let Some(record) = world.get_unit(unit_id) else {
        return;
    };
    if matches!(record.state, UnitState::Dead) {
        world.clear_locomotion_surface(unit_id);
        return;
    }
    let space_id = record.current_space_id;
    let position = record.placement.position;
    let previous = world.locomotion_surface(unit_id);
    let Some((support_y, surface)) =
        sample_locomotion_support_height(world, space_id, position, previous)
    else {
        return;
    };
    let entered_water =
        surface == LocomotionSurface::Water && previous != Some(LocomotionSurface::Water);
    world.set_locomotion_surface(unit_id, surface);

    if (position.local.0.y - support_y).abs() > 1e-5 {
        let mut grounded = position;
        grounded.local.0.y = support_y;
        let _ = world.update_unit_position(unit_id, grounded);
    }

    if entered_water {
        cancel_swimming_incompatible_actions(world, unit_catalog, weapon_catalog, unit_id);
    }
}

pub fn cancel_swimming_incompatible_actions(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    unit_id: UnitId,
) {
    let mut events = Vec::new();
    cancel_unit_task(
        world,
        unit_id,
        TaskCancelReason::Invalidated,
        &mut events,
    );
    let _ = events;
    let pending_order = world
        .command_buffer()
        .pending_for(unit_id)
        .map(|pending| pending.order);
    if pending_order.is_some_and(unit_order_requires_normal_actions) {
        world.command_buffer_mut().clear_pending(unit_id);
    }
    clear_attack_cycle_for_order_cancel(world, unit_id, None, unit_catalog, weapon_catalog);
    let _ = world.set_reactive_combat_target(unit_id, None);
    let _ = world.set_unit_combat_state(unit_id, CombatState::Peaceful);
    if matches!(
        world.get_unit(unit_id).map(|record| record.state.clone()),
        Some(UnitState::Working { .. })
    ) {
        let _ = world.set_unit_state(unit_id, UnitState::Idle);
    }
}
