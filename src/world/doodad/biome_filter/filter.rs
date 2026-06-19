use crate::world::doodad::catalog::DoodadCatalog;
use crate::world::doodad::generation::DoodadSpawnCandidate;
use crate::world::WorldData;

/// Output of [`filter_candidates_by_biome`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BiomeFilterResult {
    pub retained: Vec<DoodadSpawnCandidate>,
    pub skipped_biome_disallowed: u32,
    pub skipped_biome_unavailable: u32,
    pub skipped_invalid_definition: u32,
}

/// Pure biome membership filter (ADR-025).
///
/// Samples [`WorldData::biome_at`] for each candidate. When no biome mask is
/// loaded, all candidates are skipped — no permissive fallback.
pub fn filter_candidates_by_biome(
    candidates: &[DoodadSpawnCandidate],
    catalog: &DoodadCatalog,
    world: &WorldData,
) -> BiomeFilterResult {
    let Some(_mask) = world.biome_mask() else {
        return BiomeFilterResult {
            retained: Vec::new(),
            skipped_biome_unavailable: candidates.len() as u32,
            ..BiomeFilterResult::default()
        };
    };

    let mut result = BiomeFilterResult {
        retained: Vec::with_capacity(candidates.len()),
        ..BiomeFilterResult::default()
    };

    for candidate in candidates {
        let Some(definition) = catalog.get(&candidate.definition_id) else {
            result.skipped_invalid_definition += 1;
            continue;
        };

        let Some(sample) = world.biome_at(candidate.position) else {
            result.skipped_biome_unavailable += 1;
            continue;
        };

        if definition.allows_biome(sample.biome) {
            result.retained.push(candidate.clone());
        } else {
            result.skipped_biome_disallowed += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::biome::{BiomeColorMapping, BiomeId, BiomeMask, BiomeMaskBounds};
    use crate::world::{
        ChunkCoord, ChunkLayout, DoodadCatalog, DoodadDefinitionId, DoodadSource, LocalPosition,
        WorldData, WorldPosition,
    };
    use bevy::prelude::{Quat, Vec3};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn forest_desert_mask() -> BiomeMask {
        BiomeMask::from_rgba_rows(
            2,
            1,
            BiomeMaskBounds::new(0.0, 0.0, 512.0, 256.0),
            &[
                0, 255, 0, 255, // west half: forest
                255, 0, 0, 255, // east half: desert
            ],
            4,
            &BiomeColorMapping::starter(),
        )
        .unwrap()
    }

    fn candidate_at(x: f32, z: f32) -> DoodadSpawnCandidate {
        DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(x, 0.0, z)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    #[test]
    fn forest_tree_accepted_in_forest_pixel() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        world.set_biome_mask(forest_desert_mask());

        let result = filter_candidates_by_biome(
            &[candidate_at(64.0, 128.0)],
            &catalog,
            &world,
        );

        assert_eq!(result.retained.len(), 1);
        assert_eq!(result.skipped_biome_disallowed, 0);
    }

    #[test]
    fn desert_tree_rejected_for_forest_only_definition() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        world.set_biome_mask(forest_desert_mask());

        let result = filter_candidates_by_biome(
            &[candidate_at(400.0, 128.0)],
            &catalog,
            &world,
        );

        assert!(result.retained.is_empty());
        assert_eq!(result.skipped_biome_disallowed, 1);
    }

    #[test]
    fn missing_biome_mask_skips_all_candidates() {
        let catalog = DoodadCatalog::default();
        let world = WorldData::new(layout());

        let result = filter_candidates_by_biome(
            &[candidate_at(64.0, 128.0), candidate_at(400.0, 128.0)],
            &catalog,
            &world,
        );

        assert!(result.retained.is_empty());
        assert_eq!(result.skipped_biome_unavailable, 2);
    }

    #[test]
    fn filter_is_deterministic() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        world.set_biome_mask(forest_desert_mask());
        let candidates = vec![candidate_at(64.0, 128.0), candidate_at(400.0, 128.0)];

        let first = filter_candidates_by_biome(&candidates, &catalog, &world);
        let second = filter_candidates_by_biome(&candidates, &catalog, &world);
        assert_eq!(first, second);
    }

    #[test]
    fn allowed_biome_list_preserved_from_catalog() {
        let catalog = DoodadCatalog::default();
        let oak = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
        let rock = catalog.get(&DoodadDefinitionId::new("rock_large")).unwrap();

        assert!(oak.allows_biome(BiomeId::Forest));
        assert!(!oak.allows_biome(BiomeId::Desert));
        assert!(rock.allows_biome(BiomeId::Desert));
        assert!(rock.allows_biome(BiomeId::Marsh));
    }
}
