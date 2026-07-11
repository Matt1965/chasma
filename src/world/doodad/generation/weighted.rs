use std::collections::BTreeMap;

use super::candidate::DoodadSpawnCandidate;
use super::rng::DeterministicRng;
use crate::world::DoodadDefinitionId;
use crate::world::doodad::catalog::{DoodadCatalog, DoodadDefinition};

/// Pick one enabled definition using catalog [`DoodadDefinition::spawn_weight`] values.
///
/// Deterministic for a given RNG state. Definitions are considered in catalog order.
pub fn pick_weighted_definition<'a>(
    definitions: &[&'a DoodadDefinition],
    rng: &mut DeterministicRng,
) -> &'a DoodadDefinition {
    debug_assert!(!definitions.is_empty());

    let total_weight: f32 = definitions
        .iter()
        .map(|definition| effective_spawn_weight(definition))
        .sum();

    if total_weight <= 0.0 {
        let index = (rng.next_u32() as usize) % definitions.len();
        return definitions[index];
    }

    let mut pick = rng.next_f32() * total_weight;
    for definition in definitions {
        pick -= effective_spawn_weight(definition);
        if pick < 0.0 {
            return definition;
        }
    }

    definitions.last().expect("non-empty definitions")
}

fn effective_spawn_weight(definition: &DoodadDefinition) -> f32 {
    definition.spawn_weight.max(0.0)
}

/// Count candidates by definition id (stable key order).
pub fn count_candidates_by_definition(
    candidates: &[DoodadSpawnCandidate],
) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        *counts
            .entry(candidate.definition_id.as_str().to_string())
            .or_insert(0) += 1;
    }
    counts
}

/// Concise debug summary of candidate counts by catalog display name.
pub fn format_candidate_summary(
    candidates: &[DoodadSpawnCandidate],
    catalog: &DoodadCatalog,
) -> String {
    let counts = count_candidates_by_definition(candidates);
    if counts.is_empty() {
        return "no candidates".to_string();
    }

    counts
        .into_iter()
        .map(|(id, count)| {
            let label = catalog
                .get(&DoodadDefinitionId::new(&id))
                .map(|definition| definition.display_name.as_str())
                .unwrap_or(id.as_str());
            format!("{label}={count}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::doodad::catalog::{DoodadDefinition, DoodadRenderKey};
    use crate::world::{DoodadDefinitionId, DoodadKind};

    fn sample_definition(id: &str, weight: f32) -> DoodadDefinition {
        DoodadDefinition::new(
            DoodadDefinitionId::new(id),
            DoodadKind::Tree,
            id,
            1.0,
            1.0,
            1.0,
            None,
            None,
            None,
            true,
            DoodadRenderKey::reserved("tree/oak"),
        )
        .with_spawn_weight(weight)
    }

    #[test]
    fn weighted_pick_is_deterministic() {
        let heavy = sample_definition("heavy", 10.0);
        let light = sample_definition("light", 1.0);
        let definitions = vec![&heavy, &light];

        let mut rng_a = DeterministicRng::new(42);
        let mut rng_b = DeterministicRng::new(42);
        let picks_a: Vec<_> = (0..8)
            .map(|_| {
                pick_weighted_definition(&definitions, &mut rng_a)
                    .id
                    .as_str()
            })
            .collect();
        let picks_b: Vec<_> = (0..8)
            .map(|_| {
                pick_weighted_definition(&definitions, &mut rng_b)
                    .id
                    .as_str()
            })
            .collect();
        assert_eq!(picks_a, picks_b);
    }

    #[test]
    fn higher_weight_selected_more_often() {
        let heavy = sample_definition("heavy", 100.0);
        let light = sample_definition("light", 1.0);
        let definitions = vec![&heavy, &light];
        let mut rng = DeterministicRng::new(99);
        let mut heavy_count = 0;
        for _ in 0..200 {
            if pick_weighted_definition(&definitions, &mut rng).id.as_str() == "heavy" {
                heavy_count += 1;
            }
        }
        assert!(heavy_count > 150, "heavy_count={heavy_count}");
    }

    #[test]
    fn zero_total_weight_falls_back_to_uniform() {
        let a = sample_definition("a", 0.0);
        let b = sample_definition("b", -1.0);
        let definitions = vec![&a, &b];
        let mut rng = DeterministicRng::new(7);
        let pick = pick_weighted_definition(&definitions, &mut rng);
        assert!(pick.id.as_str() == "a" || pick.id.as_str() == "b");
    }
}
