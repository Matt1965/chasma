use super::id::SpaceId;
use super::registry::SpaceRegistry;
use crate::world::water::sample_locomotion_support_height;
use crate::world::{WorldData, WorldPosition};

/// Sample authoritative support height for grounding (ADR-083 B6).
pub fn sample_support_height(
    world: &WorldData,
    _space_registry: &SpaceRegistry,
    space_id: SpaceId,
    position: WorldPosition,
) -> Option<f32> {
    sample_locomotion_support_height(world, space_id, position, None).map(|(y, _)| y)
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
    ground_position_in_space_with_surface(world, space_registry, space_id, position, None)
}

/// Ground a position using locomotion hysteresis for a specific unit.
pub fn ground_position_in_space_with_surface(
    world: &WorldData,
    _space_registry: &SpaceRegistry,
    space_id: SpaceId,
    position: WorldPosition,
    previous: Option<crate::world::LocomotionSurface>,
) -> Option<WorldPosition> {
    let (y, _) = sample_locomotion_support_height(world, space_id, position, previous)?;
    let mut grounded = position;
    grounded.local.0.y = y;
    Some(grounded)
}
