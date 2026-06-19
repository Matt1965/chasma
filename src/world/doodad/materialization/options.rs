/// Materialization hooks (ADR-019, ADR-020, ADR-021, ADR-022, ADR-025).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterializationOptions {
    /// Filter procedural candidates against [`WorldData::biome_at`] (ADR-025).
    pub apply_biome_filter: bool,
    /// Filter procedural candidates against [`crate::world::DoodadExclusionZone`] before insert.
    pub apply_exclusion_zones: bool,
    /// Validate candidates against resident terrain height/slope before insert (ADR-021).
    pub validate_terrain: bool,
    /// Snap candidate Y to sampled terrain height before insert (ADR-022).
    pub snap_to_terrain: bool,
}

impl MaterializationOptions {
    /// Minimal pipeline: snap only (no exclusion or terrain constraint validation).
    ///
    /// Use in unit tests or callers that manage filtering externally. Prefer
    /// [`Self::procedural_default`] for production procedural materialization.
    pub fn raw() -> Self {
        Self {
            apply_biome_filter: false,
            apply_exclusion_zones: false,
            validate_terrain: false,
            snap_to_terrain: true,
        }
    }

    /// Production procedural preset: biome, exclusion, terrain validation, and snap.
    pub fn procedural_default() -> Self {
        Self {
            apply_biome_filter: true,
            apply_exclusion_zones: true,
            validate_terrain: true,
            snap_to_terrain: true,
        }
    }
}

/// [`MaterializationOptions::procedural_default`] — full procedural filter pipeline.
///
/// [`MaterializationOptions::raw`] preserves the pre-audit minimal behavior for tests.
impl Default for MaterializationOptions {
    fn default() -> Self {
        Self::procedural_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_default_enables_full_pipeline() {
        let opts = MaterializationOptions::procedural_default();
        assert!(opts.apply_biome_filter);
        assert!(opts.apply_exclusion_zones);
        assert!(opts.validate_terrain);
        assert!(opts.snap_to_terrain);
    }

    #[test]
    fn raw_disables_biome_and_exclusion_filters() {
        let opts = MaterializationOptions::raw();
        assert!(!opts.apply_biome_filter);
        assert!(!opts.apply_exclusion_zones);
        assert!(!opts.validate_terrain);
        assert!(opts.snap_to_terrain);
    }

    #[test]
    fn raw_preserves_minimal_snap_only_behavior() {
        let opts = MaterializationOptions::raw();
        assert!(!opts.apply_exclusion_zones);
        assert!(!opts.validate_terrain);
        assert!(opts.snap_to_terrain);
    }

    #[test]
    fn default_is_procedural_default() {
        assert_eq!(MaterializationOptions::default(), MaterializationOptions::procedural_default());
    }
}
