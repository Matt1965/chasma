//! Building interiors, doors, and child object authoring (ADR-084 B7).

mod activate;
mod catalog;
mod door;
mod door_store;
mod error;
mod id;
mod profile;

#[cfg(test)]
mod tests;

pub use activate::{
    activate_building_interior, deactivate_building_interior,
    refresh_building_navigation_runtime, try_activate_interior_if_complete,
};
pub use catalog::{
    DoorTemplate, InteriorChildKind, InteriorChildPlacement, InteriorProfile,
    InteriorProfileCatalog, starter_interior_profiles,
};
pub use door::{
    DoorAccessPolicy, DoorRecord, DoorState, portal_traversable_for_unit, unit_may_open_door,
};
pub use door_store::{
    DoorStore, close_door, destroy_door, lock_door, open_door, portal_traversable,
    space_route_for_unit, traversable_portals_from_space, try_open_door_at_portal_for_unit,
    try_open_door_for_unit,
};
pub use error::InteriorError;
pub use id::{DoorId, InteriorProfileId};
pub use profile::{barn_interior_profile, two_story_hut_interior_profile};
