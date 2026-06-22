use crate::world::DoodadKind;

/// Tunable counts for procedural generation (ADR-018).
///
/// Generation is pure and side-effect free: it emits [`super::candidate::DoodadSpawnCandidate`]
/// values only. Exclusion, terrain validation, and snapping run later during
/// **materialization** via [`crate::world::MaterializationOptions`], not here.
///
/// Reserved boolean/vector fields below are future planning seams for generation-time
/// hooks; they are not read by [`super::generator::generate_chunk_doodads`] today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoodadGenerationSettings {
    /// Tree-kind candidates per chunk (distributed across enabled tree definitions).
    pub trees_per_chunk: u32,
    /// Rock-kind candidates per chunk.
    pub rocks_per_chunk: u32,
    /// Bush-kind candidates per chunk.
    pub bushes_per_chunk: u32,
    /// Ruin-kind candidates per chunk.
    pub ruins_per_chunk: u32,
    /// Resource-node-kind candidates per chunk.
    pub resource_nodes_per_chunk: u32,
    /// Reserved: biome tag filter (empty = no filter). Not applied during generation.
    pub biome_filter_tags: Vec<String>,
    /// Reserved: not wired — use [`crate::world::MaterializationOptions::validate_terrain`].
    pub validate_terrain: bool,
    /// Reserved: not wired — use [`crate::world::MaterializationOptions::apply_exclusion_zones`].
    pub apply_exclusion_zones: bool,
    /// Reserved: authored-instance suppression. Not applied during generation.
    pub suppress_authored_regions: bool,
    /// Reserved: density-map modulation. Not applied during generation.
    pub use_density_maps: bool,
}

impl Default for DoodadGenerationSettings {
    /// Starter density for dev preview and unit tests (ADR-018 Phase 3D).
    ///
    /// For deliberate stress testing use [`Self::stress_test()`] — do not raise
    /// these counts in `Default`.
    fn default() -> Self {
        Self {
            trees_per_chunk: 1,
            rocks_per_chunk: 4,
            bushes_per_chunk: 6,
            ruins_per_chunk: 0,
            resource_nodes_per_chunk: 0,
            biome_filter_tags: Vec::new(),
            validate_terrain: false,
            apply_exclusion_zones: false,
            suppress_authored_regions: false,
            use_density_maps: false,
        }
    }
}

impl DoodadGenerationSettings {
    /// High candidate counts for deliberate performance / density stress tests.
    ///
    /// Not used by dev preview or production paths unless explicitly selected.
    pub fn stress_test() -> Self {
        Self {
            trees_per_chunk: 800,
            rocks_per_chunk: 40,
            bushes_per_chunk: 60,
            ..Self::default()
        }
    }

    pub fn count_for_kind(&self, kind: DoodadKind) -> u32 {
        match kind {
            DoodadKind::Tree => self.trees_per_chunk,
            DoodadKind::Rock => self.rocks_per_chunk,
            DoodadKind::Bush => self.bushes_per_chunk,
            DoodadKind::Ruin => self.ruins_per_chunk,
            DoodadKind::ResourceNode => self.resource_nodes_per_chunk,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uses_starter_density() {
        let settings = DoodadGenerationSettings::default();
        assert_eq!(settings.trees_per_chunk, 8);
        assert_eq!(settings.rocks_per_chunk, 4);
        assert_eq!(settings.bushes_per_chunk, 6);
        assert_eq!(settings.ruins_per_chunk, 0);
        assert_eq!(settings.resource_nodes_per_chunk, 0);
    }

    #[test]
    fn stress_test_is_explicit_and_higher_than_default() {
        let stress = DoodadGenerationSettings::stress_test();
        let default = DoodadGenerationSettings::default();
        assert!(stress.trees_per_chunk > default.trees_per_chunk);
        assert!(stress.rocks_per_chunk > default.rocks_per_chunk);
        assert!(stress.bushes_per_chunk > default.bushes_per_chunk);
    }
}
