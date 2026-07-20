//! Deterministic building candidate selection (SA9).

use crate::world::building::catalog::{BuildingCatalog, BuildingDefinition, BuildingDefinitionId};

use super::capacity::building_satisfies_mapping;
use super::catalog::{BuildingConstructionCostCatalog, ConstructionResponseMapping};
use super::report::BuildingCandidateScore;

/// Score eligible buildings for a construction mapping. Higher is better. Fully deterministic.
pub fn select_building_candidates(
    building_catalog: &BuildingCatalog,
    cost_catalog: &BuildingConstructionCostCatalog,
    mapping: &ConstructionResponseMapping,
) -> Vec<BuildingCandidateScore> {
    let mut scores = Vec::new();
    let candidates: Vec<&BuildingDefinition> = if !mapping.eligible_building_ids.is_empty() {
        mapping
            .eligible_building_ids
            .iter()
            .filter_map(|id| building_catalog.get(id))
            .collect()
    } else {
        building_catalog
            .definitions()
            .iter()
            .filter(|d| building_satisfies_mapping(d, mapping))
            .collect()
    };

    for definition in candidates {
        if !definition.enabled {
            continue;
        }
        if !building_satisfies_mapping(definition, mapping) {
            continue;
        }
        let (score, reasons) = score_building(definition, cost_catalog);
        scores.push(BuildingCandidateScore {
            building_definition_id: definition.id.clone(),
            score,
            reasons,
        });
    }

    scores.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| {
                a.building_definition_id
                    .as_str()
                    .cmp(b.building_definition_id.as_str())
            })
    });
    scores
}

pub fn best_building_candidate(
    building_catalog: &BuildingCatalog,
    cost_catalog: &BuildingConstructionCostCatalog,
    mapping: &ConstructionResponseMapping,
) -> Option<BuildingDefinitionId> {
    select_building_candidates(building_catalog, cost_catalog, mapping)
        .into_iter()
        .next()
        .map(|c| c.building_definition_id)
}

fn score_building(
    definition: &BuildingDefinition,
    cost_catalog: &BuildingConstructionCostCatalog,
) -> (i32, Vec<String>) {
    let mut score = 1000i32;
    let mut reasons = Vec::new();

    // Prefer buildings that already declare supporting operations.
    let ops = definition.supported_operations.len() as i32;
    score += ops * 10;
    reasons.push(format!("supported_operations={ops}"));

    // Prefer lower material cost (total quantity).
    let materials = cost_catalog.materials_for(&definition.id);
    let material_qty: u32 = materials.iter().map(|(_, q)| *q).sum();
    let cost_penalty = (material_qty as i32).min(200);
    score -= cost_penalty;
    reasons.push(format!("material_qty={material_qty} penalty={cost_penalty}"));

    // Prefer faster construction.
    let build_time = definition.build_time_seconds.max(0.0) as i32;
    let time_penalty = (build_time / 5).min(100);
    score -= time_penalty;
    reasons.push(format!("build_time={build_time} penalty={time_penalty}"));

    // Stable tie-break contribution from definition id length (tiny).
    score -= definition.id.as_str().len() as i32;
    (score, reasons)
}
