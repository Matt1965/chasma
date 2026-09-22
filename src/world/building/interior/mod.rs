//! Building interiors, doors, and child object authoring (ADR-084 B7).

mod activate;
mod catalog;
mod door;
mod door_store;
mod error;
mod id;
mod outcome;
mod profile;

#[cfg(test)]
mod tests;

pub use activate::{
    NavigationReconcileOutcome, activate_building_interior, deactivate_building_interior,
    reconcile_all_building_navigation_runtimes, reconcile_building_navigation_runtime,
    refresh_building_navigation_runtime, try_activate_interior_if_complete,
};
pub use catalog::{
    InteriorChildKind, InteriorProfile,
    InteriorProfileCatalog,
};
pub use door::{
    DoorAccessPolicy, DoorRecord, DoorState,
};
pub use door_store::{
    DoorStore, close_door, destroy_door, lock_door, open_door, portal_traversable,
    space_route_for_unit, try_open_door_at_portal_for_unit,
    try_open_door_for_unit,
};
pub use error::InteriorError;
pub use id::{DoorId, InteriorProfileId};
pub use outcome::{
    InteriorActivationOutcome, InteriorActivationOutcomeStore, InteriorActivationStatus,
};
