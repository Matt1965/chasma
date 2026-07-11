use bevy::prelude::*;

use super::candidate::DoodadSpawnCandidate;
use super::context::DoodadGenerationContext;
use super::rng::{DeterministicRng, chunk_seed};
use super::settings::DoodadGenerationSettings;
use super::weighted::pick_weighted_definition;
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
            let definition = pick_weighted_definition(&definitions, &mut rng);
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

    DoodadSpawnCandidate {
        definition_id: definition.id.clone(),
        source: DoodadSource::Procedural {
            seed: candidate_seed,
        },
        position: WorldPosition::new(
            context.chunk.coord(),
            LocalPosition::new(Vec3::new(local_x, 0.0, local_z)),
        ),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }
}

/// Stable output ordering: definition id, then local xz, then procedural seed.
fn sort_candidates(candidates: &mut [DoodadSpawnCandidate]) {
    candidates.sort_by(|a, b| {
        a.definition_id
            .cmp(&b.definition_id)
            .then_with(|| a.position.local.0.x.total_cmp(&b.position.local.0.x))
            .then_with(|| a.position.local.0.z.total_cmp(&b.position.local.0.z))
            .then_with(|| procedural_seed(a.source).cmp(&procedural_seed(b.source)))
    });
}

fn procedural_seed(source: DoodadSource) -> u64 {
    match source {
        DoodadSource::Procedural { seed } => seed,
        DoodadSource::Authored | DoodadSource::Dev => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::super::weighted::count_candidates_by_definition;
    use super::*;
    use crate::world::doodad::catalog::DoodadRenderKey;
    use crate::world::{ChunkCoord, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn generate(world_seed: u64, x: i32, z: i32) -> Vec<DoodadSpawnCandidate> {
        let layout = layout();
        let ctx =
            DoodadGenerationContext::new(world_seed, ChunkId::new(ChunkCoord::new(x, z)), &layout);
        generate_chunk_doodads(&ctx, &DoodadCatalog::default())
    }

    /// Enough tree candidates to exercise weighted distribution across definitions.
    fn distribution_settings() -> DoodadGenerationSettings {
        DoodadGenerationSettings {
            trees_per_chunk: 64,
            ..DoodadGenerationSettings::default()
        }
    }

    fn generate_for_distribution(world_seed: u64, x: i32, z: i32) -> Vec<DoodadSpawnCandidate> {
        let layout = layout();
        let ctx =
            DoodadGenerationContext::new(world_seed, ChunkId::new(ChunkCoord::new(x, z)), &layout);
        generate_chunk_doodads_with_settings(
            &ctx,
            &DoodadCatalog::default(),
            &distribution_settings(),
        )
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
            assert!(matches!(candidate.source, DoodadSource::Procedural { .. }));
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
        let settings = DoodadGenerationSettings::default();
        let tree_ids: Vec<_> = catalog
            .definitions_for_kind(DoodadKind::Tree)
            .filter(|d| d.enabled)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(tree_ids.len(), 2);

        let layout = layout();
        let ctx = DoodadGenerationContext::new(0, ChunkId::new(ChunkCoord::new(0, 0)), &layout);
        let candidates = generate_chunk_doodads_with_settings(&ctx, &catalog, &settings);
        let tree_candidates: Vec<_> = candidates
            .iter()
            .filter(|c| tree_ids.contains(&c.definition_id.as_str()))
            .collect();
        assert_eq!(tree_candidates.len(), settings.trees_per_chunk as usize);
    }

    #[test]
    fn generation_emits_identity_transform_for_finalization() {
        let candidates = generate(4242, 0, 0);
        assert!(!candidates.is_empty());
        for candidate in &candidates {
            assert_eq!(candidate.rotation, Quat::IDENTITY);
            assert_eq!(candidate.scale, Vec3::ONE);
        }
    }

    #[test]
    fn weighted_selection_is_deterministic_for_seed() {
        let counts_a = count_candidates_by_definition(&generate_for_distribution(4242, 3, 4));
        let counts_b = count_candidates_by_definition(&generate_for_distribution(4242, 3, 4));
        assert_eq!(counts_a, counts_b);
        assert!(counts_a.get("tree_oak").copied().unwrap_or(0) > 0);
        assert!(counts_a.get("tree_dead").copied().unwrap_or(0) > 0);
    }

    #[test]
    fn different_seeds_change_definition_distribution() {
        let counts_a = count_candidates_by_definition(&generate_for_distribution(1, 8, 8));
        let counts_b = count_candidates_by_definition(&generate_for_distribution(2, 8, 8));
        assert_ne!(counts_a, counts_b);
    }

    #[test]
    fn spawn_weights_influence_tree_distribution() {
        let heavy = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_heavy"),
            DoodadKind::Tree,
            "Heavy Tree",
            1.0,
            1.0,
            1.0,
            None,
            None,
            None,
            true,
            DoodadRenderKey::reserved("tree/oak"),
        )
        .with_spawn_weight(20.0);
        let light = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_light"),
            DoodadKind::Tree,
            "Light Tree",
            1.0,
            1.0,
            1.0,
            None,
            None,
            None,
            true,
            DoodadRenderKey::reserved("tree/dead"),
        )
        .with_spawn_weight(1.0);
        let catalog = DoodadCatalog::from_definitions(vec![heavy, light]).expect("valid catalog");
        let settings = DoodadGenerationSettings {
            trees_per_chunk: 128,
            rocks_per_chunk: 0,
            bushes_per_chunk: 0,
            ..DoodadGenerationSettings::default()
        };
        let layout = layout();
        let ctx = DoodadGenerationContext::new(7, ChunkId::new(ChunkCoord::new(0, 0)), &layout);
        let candidates = generate_chunk_doodads_with_settings(&ctx, &catalog, &settings);
        let heavy_count = candidates
            .iter()
            .filter(|c| c.definition_id.as_str() == "tree_heavy")
            .count();
        let light_count = candidates
            .iter()
            .filter(|c| c.definition_id.as_str() == "tree_light")
            .count();
        assert!(heavy_count > light_count);
        assert!(light_count >= 2);
    }

    #[test]
    fn disabled_definitions_are_never_selected() {
        let disabled = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_disabled"),
            DoodadKind::Tree,
            "Disabled Tree",
            1.0,
            1.0,
            1.0,
            None,
            None,
            None,
            false,
            DoodadRenderKey::reserved("tree/dead"),
        )
        .with_spawn_weight(100.0);
        let enabled = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_enabled"),
            DoodadKind::Tree,
            "Enabled Tree",
            1.0,
            1.0,
            1.0,
            None,
            None,
            None,
            true,
            DoodadRenderKey::reserved("tree/oak"),
        )
        .with_spawn_weight(1.0);
        let catalog =
            DoodadCatalog::from_definitions(vec![disabled, enabled]).expect("valid catalog");
        let settings = DoodadGenerationSettings {
            trees_per_chunk: 32,
            rocks_per_chunk: 0,
            bushes_per_chunk: 0,
            ..DoodadGenerationSettings::default()
        };
        let layout = layout();
        let ctx = DoodadGenerationContext::new(1, ChunkId::new(ChunkCoord::new(0, 0)), &layout);
        let candidates = generate_chunk_doodads_with_settings(&ctx, &catalog, &settings);
        assert!(
            candidates
                .iter()
                .all(|c| c.definition_id.as_str() == "tree_enabled")
        );
    }

    #[test]
    fn starter_catalog_produces_multiple_forest_kinds() {
        let candidates = generate_for_distribution(9001, 5, 5);
        let ids: std::collections::BTreeSet<_> = candidates
            .iter()
            .map(|c| c.definition_id.as_str())
            .collect();
        assert!(ids.contains("tree_oak"));
        assert!(ids.contains("tree_dead"));
        assert!(ids.contains("bush_scrub"));
        assert!(ids.contains("rock_small"));
    }
}
