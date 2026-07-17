//! Generalized static occupancy and baked footprints (ADR-080 B3).

mod catalog;
mod cell;
mod ellipse;
mod error;
mod footprint;
mod grid;
mod passability;
mod query;
mod registration;

#[cfg(test)]
pub mod test_support;

#[cfg(any(test, feature = "data-import"))]
pub mod bake;

pub use catalog::{FootprintCatalog, FootprintCatalogError, FootprintId};
pub use cell::{
    OCCUPANCY_CELL_SIZE_METERS, OccupancyCellCoord, QuantizedRotation, SURFACE_SPACE_ID,
    chunk_for_occupancy_cell, circle_overlap_blocked, occupancy_cell_at_global_xz,
};
pub use error::{OccupancyError, OccupancySource, conservative_block_radius_for_kind};
pub use footprint::{
    BakedCellMask, FootprintDefinition, FootprintShape, agent_overlaps_footprint,
    agent_overlaps_footprint_continuous, effective_building_footprint,
    effective_building_footprint_for_placement, inline_building_footprint,
    inline_footprint_from_building, occupied_cells_for_footprint, occupied_cells_for_footprint_yaw,
    scale_footprint_shape,
};
pub use grid::{ChunkOccupancyGrid, OccupancyCellEntry, OccupancyState, default_space_id};
pub use passability::{
    PassabilityAgent, PassabilityBlockReason, PassabilityCatalogs, PassabilityResult,
    PassabilityUnavailableReason, is_position_blocked_for_agent, is_position_passable,
    query_passability_at, query_passability_in_space,
};
pub use query::{
    StaticOccupancyResult, is_position_blocked_by_static_occupancy, query_static_occupancy_at,
};
pub use registration::{
    DoodadRegistrationOptions, OccupancyCatalogs, OccupancyRegistrationPlan,
    apply_registration_plan, plan_register_building, plan_register_doodad, plan_unregister_source,
    rebuild_occupancy_index, register_building_occupancy, register_doodad_occupancy,
    unregister_source_occupancy, update_building_occupancy, update_doodad_occupancy,
};

#[cfg(any(test, feature = "dev"))]
pub use catalog::starter_footprint_definitions;

#[cfg(feature = "dev")]
pub use crate::data_import::resolve_dev_footprint_catalog;
