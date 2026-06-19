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
    fn default() -> Self {
        Self {
            trees_per_chunk: 8,
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
