//! Reserved future exclusion filter hooks (ADR-020).
//!
//! Phase 3F exclusion is implemented via [`filter_candidates_by_exclusion_zones`]
//! during the **materialization pipeline** ([`crate::world::MaterializationOptions`]).
//! Fields here are planning seams only — they are not read by generation or
//! materialization today.

/// Reserved for polygon zones, biome filters, and future exclusion shape dispatch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExclusionFilterOptions {
    /// Reserved: polygonal exclusion regions. Not implemented in Phase 3F.
    pub polygon_zones: Vec<String>,
    /// Reserved: not wired — use [`crate::world::MaterializationOptions::validate_terrain`].
    pub validate_terrain: bool,
    /// Reserved: biome-aware exclusion. Not implemented.
    pub biome_filter: bool,
    /// Reserved: authored suppression regions. Not implemented.
    pub suppress_authored_regions: bool,
}
