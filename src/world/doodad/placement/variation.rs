//! Catalog-driven scale and yaw applied during placement finalization (R7).
//!
//! [`DoodadSpawnCandidate`] values are never mutated; believability is applied only
//! to [`super::FinalizedDoodadPlacement`] using catalog [`DoodadDefinition`] fields.

use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::world::doodad::catalog::{DoodadCatalog, DoodadDefinition};
use crate::world::doodad::generation::{chunk_seed, DeterministicRng};
use crate::world::doodad::source::DoodadSource;
use crate::world::{ChunkCoord, DoodadKind};

use super::FinalizedDoodadPlacement;

/// Summary of believability applied during finalization (dev diagnostics).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlacementBelievabilitySummary {
    pub placements_applied: u32,
    pub rotation_randomized: u32,
    pub rotation_identity: u32,
    pub scale_min: Option<f32>,
    pub scale_max: Option<f32>,
    pub counts_by_kind: HashMap<DoodadKind, u32>,
}

/// Derive a stable RNG seed from chunk coordinates, procedural instance seed, and definition id.
pub fn believability_seed(chunk: ChunkCoord, procedural_seed: u64, definition_id: &str) -> u64 {
    let base = chunk_seed(procedural_seed, chunk.x, chunk.z);
    let mut h = base ^ 0xD00D_AD17_0000_0007;
    for byte in definition_id.as_bytes() {
        h = h
            .wrapping_mul(0x100000001B3)
            .wrapping_add(u64::from(*byte));
    }
    h
}

pub fn compute_uniform_scale(definition: &DoodadDefinition, rng: &mut DeterministicRng) -> f32 {
    if (definition.max_scale - definition.min_scale).abs() < f32::EPSILON {
        return definition.min_scale;
    }
    let t = rng.next_f32();
    definition.min_scale + t * (definition.max_scale - definition.min_scale)
}

pub fn compute_yaw(rng: &mut DeterministicRng) -> f32 {
    rng.next_f32() * TAU
}

/// Apply catalog-driven scale and optional yaw to one finalized procedural placement.
pub fn apply_catalog_believability(
    placement: &mut FinalizedDoodadPlacement,
    definition: &DoodadDefinition,
) -> bool {
    let DoodadSource::Procedural { seed } = placement.source else {
        return false;
    };

    let mut rng = DeterministicRng::new(believability_seed(
        placement.position.chunk,
        seed,
        definition.id.as_str(),
    ));

    placement.scale = Vec3::splat(compute_uniform_scale(definition, &mut rng));
    placement.rotation = if definition.random_rotation_y {
        Quat::from_rotation_y(compute_yaw(&mut rng))
    } else {
        Quat::IDENTITY
    };

    true
}

/// Apply believability to all placements that resolve in `catalog`.
pub fn apply_catalog_believability_batch(
    placements: &mut [FinalizedDoodadPlacement],
    catalog: &DoodadCatalog,
) -> PlacementBelievabilitySummary {
    let mut summary = PlacementBelievabilitySummary::default();

    for placement in placements.iter_mut() {
        let Some(definition) = catalog.get(&placement.definition_id) else {
            continue;
        };

        if !apply_catalog_believability(placement, definition) {
            continue;
        }

        summary.placements_applied += 1;
        *summary.counts_by_kind.entry(definition.kind).or_insert(0) += 1;

        let scale = placement.scale.x;
        summary.scale_min = Some(summary.scale_min.map_or(scale, |m| m.min(scale)));
        summary.scale_max = Some(summary.scale_max.map_or(scale, |m| m.max(scale)));

        if definition.random_rotation_y {
            summary.rotation_randomized += 1;
        } else {
            summary.rotation_identity += 1;
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::doodad::catalog::{DoodadDefinition, DoodadRenderKey};
    use crate::world::doodad::source::DoodadSource;
    use crate::world::{ChunkCoord, DoodadDefinitionId, LocalPosition, WorldPosition};

    fn tree_definition(min: f32, max: f32, random_y: bool) -> DoodadDefinition {
        DoodadDefinition::new(
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            "Oak",
            4.0,
            min,
            max,
            None,
            None,
            Some(25.0),
            true,
            DoodadRenderKey::reserved("tree/oak"),
        )
        .with_random_rotation_y(random_y)
    }

    fn placement_at(seed: u64, x: f32, z: f32) -> FinalizedDoodadPlacement {
        FinalizedDoodadPlacement {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed },
            position: WorldPosition::new(
                ChunkCoord::new(2, 3),
                LocalPosition::new(Vec3::new(x, 0.0, z)),
            ),
            rotation: Quat::from_rotation_y(0.25),
            scale: Vec3::splat(2.0),
        }
    }

    #[test]
    fn scale_variation_is_deterministic_per_seed() {
        let definition = tree_definition(0.8, 1.2, false);
        let mut a = placement_at(99, 64.0, 96.0);
        let mut b = placement_at(99, 64.0, 96.0);
        apply_catalog_believability(&mut a, &definition);
        apply_catalog_believability(&mut b, &definition);
        assert_eq!(a.scale, b.scale);
        assert!(a.scale.x >= 0.8 && a.scale.x <= 1.2);
    }

    #[test]
    fn rotation_variation_is_deterministic_per_seed() {
        let definition = tree_definition(1.0, 1.0, true);
        let mut a = placement_at(7, 128.0, 128.0);
        let mut b = placement_at(7, 128.0, 128.0);
        apply_catalog_believability(&mut a, &definition);
        apply_catalog_believability(&mut b, &definition);
        assert_eq!(a.rotation, b.rotation);
        assert_ne!(a.rotation, Quat::IDENTITY);
    }

    #[test]
    fn rotation_disabled_uses_identity() {
        let definition = tree_definition(1.0, 1.0, false);
        let mut placement = placement_at(5, 128.0, 128.0);
        apply_catalog_believability(&mut placement, &definition);
        assert_eq!(placement.rotation, Quat::IDENTITY);
    }

    #[test]
    fn different_seeds_produce_different_scale() {
        let definition = tree_definition(0.5, 1.5, false);
        let mut a = placement_at(1, 80.0, 80.0);
        let mut b = placement_at(2, 80.0, 80.0);
        apply_catalog_believability(&mut a, &definition);
        apply_catalog_believability(&mut b, &definition);
        assert_ne!(a.scale, b.scale);
    }

    #[test]
    fn fixed_scale_when_min_equals_max() {
        let definition = tree_definition(1.2, 1.2, false);
        let mut placement = placement_at(3, 80.0, 80.0);
        apply_catalog_believability(&mut placement, &definition);
        assert!((placement.scale.x - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn different_seeds_produce_different_rotation() {
        let definition = tree_definition(1.0, 1.0, true);
        let mut a = placement_at(1, 80.0, 80.0);
        let mut b = placement_at(2, 80.0, 80.0);
        apply_catalog_believability(&mut a, &definition);
        apply_catalog_believability(&mut b, &definition);
        assert_ne!(a.rotation, b.rotation);
    }

    #[test]
    fn batch_summary_counts_rotation_modes() {
        let catalog = DoodadCatalog::from_definitions(vec![
            tree_definition(0.9, 1.1, true),
            DoodadDefinition::new(
                DoodadDefinitionId::new("rock_small"),
                DoodadKind::Rock,
                "Rock",
                3.0,
                0.8,
                1.2,
                None,
                None,
                Some(40.0),
                true,
                DoodadRenderKey::reserved("rock/small"),
            ),
        ])
        .unwrap();
        let mut placements = vec![
            placement_at(1, 64.0, 64.0),
            FinalizedDoodadPlacement {
                definition_id: DoodadDefinitionId::new("rock_small"),
                source: DoodadSource::Procedural { seed: 2 },
                position: placement_at(2, 96.0, 96.0).position,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        ];
        let summary = apply_catalog_believability_batch(&mut placements, &catalog);
        assert_eq!(summary.placements_applied, 2);
        assert_eq!(summary.rotation_randomized, 1);
        assert_eq!(summary.rotation_identity, 1);
    }
}
