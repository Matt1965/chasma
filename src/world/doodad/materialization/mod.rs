//! Candidate → instance materialization (ADR-019, Phase 3E).
//!
//! Converts [`crate::world::doodad::generation::DoodadSpawnCandidate`] output into
//! [`crate::world::WorldData`] records via the authoring API. Explicit and on-demand —
//! not an automatic runtime system.

mod materialize;
mod options;
mod report;

pub use materialize::{
    materialize_candidates, materialize_candidates_with_exclusion,
    materialize_candidates_with_options,
};
pub use options::MaterializationOptions;
pub use report::DoodadMaterializationReport;
