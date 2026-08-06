use bevy::prelude::*;

use super::definition::SpaceRecord;
use super::id::SpaceId;
use super::registry::SpaceRegistry;
use crate::world::{WorldData, WorldPosition, ground_world_position};

/// Sample authoritative support height for grounding (ADR-083 B6).
pub fn sample_support_height(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    space_id: SpaceId,
    position: WorldPosition,
) -> Option<f32> {
    if space_id.is_surface() {
        return ground_world_position(world, position).map(|grounded| grounded.local.0.y);
    }
    let space = space_registry.get_space(space_id)?;
    if !space.enabled || !space.walkable {
        return None;
    }
    Some(space.floor_y_global)
}

/// Authoritative Y that heights in `space_id` are measured above.
///
/// Terrain-derived spaces return `None`: their heights are heightfield samples and
/// presentation exaggerates them (ADR-010). An interior space returns its owning
/// building's anchor Y, because its floor is an authored metric offset above that
/// anchor — the same offset the building model carries in its own geometry. Callers
/// that place render entities must keep that offset metric or interior objects fly
/// off by `offset * (vertical_scale - 1)` (IN-11c).
pub fn space_vertical_reference_y(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    space_id: SpaceId,
) -> Option<f32> {
    if space_id.is_surface() {
        return None;
    }
    let building_id = space_registry.get_space(space_id)?.owning_building_id?;
    let building = world.get_building(building_id)?;
    Some(building.placement.position.to_global(world.layout()).y)
}

/// Ground a position within a space.
pub fn ground_position_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    space_id: SpaceId,
    position: WorldPosition,
) -> Option<WorldPosition> {
    let y = sample_support_height(world, space_registry, space_id, position)?;
    let mut grounded = position;
    grounded.local.0.y = y;
    Some(grounded)
}
