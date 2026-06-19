//! Doodad placement types and procedural placement finalization (ADR-015, ADR-022).
//!
//! [`DoodadPlacement`] is the authoritative instance pose on [`DoodadRecord`].
//! [`FinalizedDoodadPlacement`] is the procedural pipeline output consumed by
//! materialization after terrain validation.

mod finalize;
mod finalized;
mod pose;

pub use finalize::{finalize_placements, PlacementFinalizationResult};
pub use finalized::FinalizedDoodadPlacement;
pub use pose::DoodadPlacement;
