//! Shared imports for navigation blueprint integration tests.

pub use super::definition::{
    NavigationVerticalTransitionDefinition, NavigationVerticalTransitionKind,
};
pub use super::fixtures::{
    corridor_hut_navigation_blueprint, dual_doorway_navigation_blueprint,
    one_region_doorless_navigation_blueprint, two_floor_two_room_navigation_blueprint,
    two_room_hut_navigation_blueprint,
};
pub use super::opening_geometry::{
    min_interior_closed_boundary_clearance_meters, usable_center_opening_interval_on_edge,
};
pub use super::runtime::{
    interior_agent_fits_region, interior_navigation_move_target_at_position,
    interior_position_walkable, min_edge_clearance_meters, resolve_navigation_space_at_position,
    resolve_navigation_start_space,
};
pub use super::validate_inspection::validate_blueprint_for_inspection;
pub use crate::world::{PassabilityAgent, PassabilityResult};
pub use crate::world::unit::{UnitOrder, UnitSource, UnitState, create_unit, step_unit_movement};
pub use crate::world::{
    DoorState, PassabilityCatalogs, UnitDefinitionId, close_door, find_path_with_spaces,
    open_door, query_navigation_point_legality, resolve_interaction_to_order,
};
