//! Exclusion zone data and procedural candidate filtering (ADR-015, ADR-020).

mod filter;
mod options;
mod zone;

pub use filter::{ExclusionFilterResult, filter_candidates_by_exclusion_zones};
pub use options::ExclusionFilterOptions;
pub use zone::DoodadExclusionZone;
