/// Outcome counters for materialization (ADR-019, ADR-020, ADR-021, ADR-022).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DoodadMaterializationReport {
    pub candidates_received: u32,
    pub inserted: u32,
    pub excluded_by_zone: u32,
    pub skipped_terrain_unavailable: u32,
    pub skipped_height_constraint: u32,
    pub skipped_slope_constraint: u32,
    pub skipped_slope_unavailable: u32,
    pub placements_finalized: u32,
    pub terrain_snaps_applied: u32,
    pub skipped_duplicate: u32,
    pub skipped_invalid_definition: u32,
    pub skipped_disabled_definition: u32,
    pub skipped_validation_failed: u32,
}

impl DoodadMaterializationReport {
    /// Skips during the insert loop only (duplicate, invalid definition, scale, etc.).
    pub fn skipped_at_insert(&self) -> u32 {
        self.skipped_duplicate
            + self.skipped_invalid_definition
            + self.skipped_disabled_definition
            + self.skipped_validation_failed
    }

    /// All candidates not inserted, including pre-insert pipeline filters.
    pub fn skipped_total(&self) -> u32 {
        self.excluded_by_zone
            + self.skipped_terrain_unavailable
            + self.skipped_height_constraint
            + self.skipped_slope_constraint
            + self.skipped_slope_unavailable
            + self.skipped_at_insert()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skipped_at_insert_counts_insert_stage_only() {
        let report = DoodadMaterializationReport {
            excluded_by_zone: 2,
            skipped_terrain_unavailable: 3,
            skipped_duplicate: 1,
            skipped_invalid_definition: 1,
            ..DoodadMaterializationReport::default()
        };
        assert_eq!(report.skipped_at_insert(), 2);
    }

    #[test]
    fn skipped_total_includes_pipeline_and_insert_skips() {
        let report = DoodadMaterializationReport {
            excluded_by_zone: 1,
            skipped_terrain_unavailable: 2,
            skipped_height_constraint: 3,
            skipped_slope_constraint: 4,
            skipped_slope_unavailable: 5,
            skipped_duplicate: 6,
            skipped_invalid_definition: 7,
            skipped_disabled_definition: 8,
            skipped_validation_failed: 9,
            ..DoodadMaterializationReport::default()
        };
        assert_eq!(report.skipped_total(), 45);
    }
}
