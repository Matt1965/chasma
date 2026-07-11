//! Doodad placement types and procedural placement finalization (ADR-015, ADR-022).
//!
//! [`DoodadPlacement`] is the authoritative instance pose on [`DoodadRecord`].
//! [`FinalizedDoodadPlacement`] is the procedural pipeline output consumed by
//! materialization after terrain validation.

mod finalize;
mod finalized;
mod pose;
mod variation;

pub use finalize::{PlacementFinalizationResult, finalize_placements};
pub use finalized::FinalizedDoodadPlacement;
pub use pose::DoodadPlacement;
#[allow(unused_imports)]
pub use variation::{
    PlacementBelievabilitySummary, apply_catalog_believability, apply_catalog_believability_batch,
};
