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
