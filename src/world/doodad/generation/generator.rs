use std::f32::consts::TAU;

use bevy::prelude::*;

use super::candidate::DoodadSpawnCandidate;
use super::context::DoodadGenerationContext;
use super::rng::{chunk_seed, DeterministicRng};
use super::settings::DoodadGenerationSettings;
use crate::world::doodad::catalog::{DoodadCatalog, DoodadDefinition};
use crate::world::{DoodadKind, DoodadSource, LocalPosition, WorldPosition};

const KIND_ORDER: [DoodadKind; 5] = [
    DoodadKind::Tree,
    DoodadKind::Rock,
    DoodadKind::Bush,
    DoodadKind::Ruin,
    DoodadKind::ResourceNode,
];

/// Generate procedural doodad candidates for one chunk (ADR-018).
///
/// Pure, deterministic, side-effect free. Does not read or mutate
/// [`crate::world::WorldData`].
pub fn generate_chunk_doodads(
    context: &DoodadGenerationContext<'_>,
    catalog: &DoodadCatalog,
) -> Vec<DoodadSpawnCandidate> {
    generate_chunk_doodads_with_settings(context, catalog, &DoodadGenerationSettings::default())
}

/// Generate with explicit settings (counts and reserved future hooks).
pub fn generate_chunk_doodads_with_settings(
    context: &DoodadGenerationContext<'_>,
    catalog: &DoodadCatalog,
    settings: &DoodadGenerationSettings,
) -> Vec<DoodadSpawnCandidate> {
    let coord = context.chunk.coord();
    let seed = chunk_seed(context.world_seed, coord.x, coord.z);
    let mut rng = DeterministicRng::new(seed);
    let mut candidates = Vec::new();
    let chunk_size = context.layout.chunk_size_units();

    for kind in KIND_ORDER {
        let count = settings.count_for_kind(kind);
        if count == 0 {
            continue;
        }

        let definitions = enabled_definitions_for_kind(catalog, kind);
        if definitions.is_empty() {
            continue;
        }

        for _ in 0..count {
            let def_index = (rng.next_u32() as usize) % definitions.len();
            let definition = definitions[def_index];
            let candidate_seed = rng.next_u64();
            candidates.push(spawn_candidate(
                definition,
                context,
                chunk_size,
                candidate_seed,
                &mut rng,
            ));
        }
    }

    sort_candidates(&mut candidates);
    candidates
}

fn enabled_definitions_for_kind(
    catalog: &DoodadCatalog,
    kind: DoodadKind,
) -> Vec<&DoodadDefinition> {
    catalog
        .definitions_for_kind(kind)
        .filter(|definition| definition.enabled)
        .collect()
}

fn spawn_candidate(
    definition: &DoodadDefinition,
    context: &DoodadGenerationContext<'_>,
    chunk_size: f32,
    candidate_seed: u64,
    rng: &mut DeterministicRng,
) -> DoodadSpawnCandidate {
    let margin = definition.placement_radius_meters.max(1.0);
    let span = (chunk_size - margin * 2.0).max(1.0);
    let local_x = margin + rng.next_f32() * span;
    let local_z = margin + rng.next_f32() * span;
    let scale_t = rng.next_f32();
    let uniform_scale =
        definition.min_scale + scale_t * (definition.max_scale - definition.min_scale);
    let yaw = rng.next_f32() * TAU;

    DoodadSpawnCandidate {
        definition_id: definition.id.clone(),
        source: DoodadSource::Procedural { seed: candidate_seed },
        position: WorldPosition::new(
            context.chunk.coord(),
            LocalPosition::new(Vec3::new(local_x, 0.0, local_z)),
        ),
        rotation: Quat::from_rotation_y(yaw),
        scale: Vec3::splat(uniform_scale),
    }
}

/// Stable output ordering: definition id, then local xz, then procedural seed.
fn sort_candidates(candidates: &mut [DoodadSpawnCandidate]) {
    candidates.sort_by(|a, b| {
        a.definition_id
            .cmp(&b.definition_id)
            .then_with(|| {
                a.position
                    .local
                    .0
                    .x
                    .total_cmp(&b.position.local.0.x)
            })
            .then_with(|| {
                a.position
                    .local
                    .0
                    .z
                    .total_cmp(&b.position.local.0.z)
            })
            .then_with(|| procedural_seed(a.source).cmp(&procedural_seed(b.source)))
    });
}

fn procedural_seed(source: DoodadSource) -> u64 {
    match source {
        DoodadSource::Procedural { seed } => seed,
        DoodadSource::Authored => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkId, ChunkLayout, DoodadCatalog};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn generate(world_seed: u64, x: i32, z: i32) -> Vec<DoodadSpawnCandidate> {
        let layout = layout();
        let ctx = DoodadGenerationContext::new(
            world_seed,
            ChunkId::new(ChunkCoord::new(x, z)),
            &layout,
        );
        generate_chunk_doodads(&ctx, &DoodadCatalog::default())
    }

    #[test]
    fn same_seed_and_chunk_produces_identical_results() {
        let a = generate(12345, 0, 0);
        let b = generate(12345, 0, 0);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn same_seed_different_chunk_produces_different_results() {
        let a = generate(999, 0, 0);
        let b = generate(999, 1, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn different_seed_produces_different_results() {
        let a = generate(1, 5, 5);
        let b = generate(2, 5, 5);
        assert_ne!(a, b);
    }

    #[test]
    fn all_candidates_reference_valid_catalog_definitions() {
        let catalog = DoodadCatalog::default();
        let layout = layout();
        let ctx = DoodadGenerationContext::new(42, ChunkId::new(ChunkCoord::new(0, 0)), &layout);
        let candidates = generate_chunk_doodads(&ctx, &catalog);

        assert!(!candidates.is_empty());
        for candidate in &candidates {
            assert!(
                catalog.get(&candidate.definition_id).is_some(),
                "unknown definition {:?}",
                candidate.definition_id
            );
            assert!(matches!(
                candidate.source,
                DoodadSource::Procedural { .. }
            ));
        }
    }

    #[test]
    fn generation_is_side_effect_free() {
        let catalog = DoodadCatalog::default();
        let layout = layout();
        let ctx = DoodadGenerationContext::new(7, ChunkId::new(ChunkCoord::new(2, 3)), &layout);
        let first = generate_chunk_doodads(&ctx, &catalog);
        let second = generate_chunk_doodads(&ctx, &catalog);
        assert_eq!(first, second);
        assert_eq!(catalog.len(), DoodadCatalog::default().len());
    }

    #[test]
    fn generation_produces_stable_ordering() {
        let a = generate(555, 10, 10);
        let b = generate(555, 10, 10);
        let ids_a: Vec<_> = a
            .iter()
            .map(|c| (c.definition_id.as_str(), procedural_seed(c.source)))
            .collect();
        let ids_b: Vec<_> = b
            .iter()
            .map(|c| (c.definition_id.as_str(), procedural_seed(c.source)))
            .collect();
        assert_eq!(ids_a, ids_b);

        for window in a.windows(2) {
            let (left, right) = (&window[0], &window[1]);
            assert!(left.definition_id <= right.definition_id);
        }
    }

    #[test]
    fn candidate_count_is_deterministic() {
        let settings = DoodadGenerationSettings::default();
        let expected = settings.trees_per_chunk
            + settings.rocks_per_chunk
            + settings.bushes_per_chunk
            + settings.ruins_per_chunk
            + settings.resource_nodes_per_chunk;

        let layout = layout();
        let ctx = DoodadGenerationContext::new(0, ChunkId::new(ChunkCoord::new(0, 0)), &layout);
        let candidates =
            generate_chunk_doodads_with_settings(&ctx, &DoodadCatalog::default(), &settings);

        assert_eq!(candidates.len(), expected as usize);

        let again =
            generate_chunk_doodads_with_settings(&ctx, &DoodadCatalog::default(), &settings);
        assert_eq!(candidates.len(), again.len());
    }

    #[test]
    fn uses_catalog_not_hardcoded_definition_ids() {
        let catalog = DoodadCatalog::default();
        let tree_ids: Vec<_> = catalog
            .definitions_for_kind(DoodadKind::Tree)
            .filter(|d| d.enabled)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(tree_ids.len(), 2);

        let candidates = generate(0, 0, 0);
        let tree_candidates: Vec<_> = candidates
            .iter()
            .filter(|c| tree_ids.contains(&c.definition_id.as_str()))
            .collect();
        assert_eq!(tree_candidates.len(), 8);
    }
}
