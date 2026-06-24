//! Unit data layer (ADR-027).
//!
//! U1 owns type definitions in [`catalog::UnitCatalog`]. U2 adds authoritative
//! instance records on [`crate::world::WorldData`]. Runtime ECS sync (U3+) and
//! full simulation via [`UnitSimulationState`] subfields are deferred.
//!
//! Obstacle and navigation systems will live under `world/navigation/` or
//! `world/obstacle/` — not under this module.

mod authoring;
mod catalog;
mod grounding;
mod id;
mod metadata;
mod movement;
mod orders;
mod placement;
mod query;
mod record;
mod source;
mod state;
mod store;

pub use authoring::{
    create_unit, create_unit_with_ownership, lookup_unit, move_unit, remove_unit,
    UnitAuthoringError,
};
pub use catalog::{
    UnitCatalog, UnitCatalogError, UnitDefinition, UnitDefinitionId, UnitRenderKey,
};
#[cfg(test)]
pub use catalog::starter_definitions;
pub use grounding::{
    ground_unit_position, ground_unit_to_terrain, UnitGroundingError,
};
pub use movement::{
    step_all_unit_movement, step_unit_movement, BatchUnitMovementReport, UnitMovementError,
    UnitMovementStepReport,
};
pub use orders::{
    issue_unit_order, resolve_all_pending_unit_orders, resolve_pending_unit_orders, UnitOrder,
    UnitOrderError,
};
pub use id::UnitId;
pub use metadata::UnitMetadata;
pub use placement::UnitPlacement;
pub use record::UnitRecord;
pub use source::UnitSource;
pub use state::UnitState;
pub use store::ChunkUnitStore;

/// Why [`crate::world::WorldData::insert_unit`] or relocation rejected a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitInsertError {
    /// [`UnitRecord::placement`] chunk does not match the target [`crate::world::ChunkId`].
    ChunkPlacementMismatch,
    /// No unit with the given id exists in world data.
    UnitNotFound,
}

/// Future full simulation envelope (U3+). Not stored separately in U2; [`UnitState`]
/// on [`UnitRecord`] is the minimal placeholder until orders, combat, and AI arrive.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitSimulationState;

