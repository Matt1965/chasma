//! Deterministic visual variation applied after placement finalization (R7).
//!
//! Scale, optional yaw, and micro position jitter are derived from the procedural
//! instance seed and definition metadata. Generation candidate poses are not mutated;
//! this layer runs during materialization before records are authored.

use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::world::doodad::catalog::{DoodadCatalog, DoodadDefinition};
use crate::world::doodad::generation::{chunk_seed, DeterministicRng};
use crate::world::doodad::source::DoodadSource;
use crate::world::{ChunkCoord, ChunkLayout, DoodadKind};

use super::FinalizedDoodadPlacement;

const MICRO_JITTER_RADIUS_FRACTION: f32 = 0.12;
const MICRO_JITTER_ABS_MAX_METERS: f32 = 0.35;

/// Summary of variation applied to a batch of finalized placements (dev diagnostics).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlacementVariationSummary {
    pub placements_varied: u32,
    pub rotation_randomized: u32,
    pub rotation_identity: u32,
    pub scale_min: Option<f32>,
    pub scale_max: Option<f32>,
    pub counts_by_kind: HashMap<DoodadKind, u32>,
}

/// Derive a stable RNG seed from chunk coordinates, procedural instance seed, and definition id.
pub fn variation_seed(chunk: ChunkCoord, procedural_seed: u64, definition_id: &str) -> u64 {
    let base = chunk_seed(procedural_seed, chunk.x, chunk.z);
    let mut h = base ^ 0xD00D_AD17_0000_0007;
    for byte in definition_id.as_bytes() {
        h = h
            .wrapping_mul(0x100000001B3)
            .wrapping_add(u64::from(*byte));
    }
    h
}

pub fn wants_random_rotation_y(definition: &DoodadDefinition) -> bool {
    definition
        .placement_tags
        .iter()
        .any(|tag| tag == "random_rotation_y")
}

pub fn compute_uniform_scale(definition: &DoodadDefinition, rng: &mut DeterministicRng) -> f32 {
    let t = rng.next_f32();
    definition.min_scale + t * (definition.max_scale - definition.min_scale)
}

pub fn compute_yaw(rng: &mut DeterministicRng) -> f32 {
    rng.next_f32() * TAU
}

pub fn apply_micro_jitter(
    local_x: f32,
    local_z: f32,
    margin: f32,
    chunk_size: f32,
    placement_radius: f32,
    rng: &mut DeterministicRng,
) -> (f32, f32) {
    let max_jitter =
        (placement_radius * MICRO_JITTER_RADIUS_FRACTION).min(MICRO_JITTER_ABS_MAX_METERS);
    if max_jitter <= 0.0 {
        return (local_x, local_z);
    }
    let dx = (rng.next_f32() * 2.0 - 1.0) * max_jitter;
    let dz = (rng.next_f32() * 2.0 - 1.0) * max_jitter;
    let min = margin;
    let max = (chunk_size - margin).max(min);
    ((local_x + dx).clamp(min, max), (local_z + dz).clamp(min, max))
}

/// Apply visual variation to one finalized procedural placement.
pub fn apply_placement_variation(
    placement: &mut FinalizedDoodadPlacement,
    definition: &DoodadDefinition,
    layout: ChunkLayout,
) -> bool {
    let DoodadSource::Procedural { seed } = placement.source else {
        return false;
    };

    let mut rng = DeterministicRng::new(variation_seed(
        placement.position.chunk,
        seed,
        definition.id.as_str(),
    ));

    let uniform_scale = compute_uniform_scale(definition, &mut rng);
    placement.scale = Vec3::splat(uniform_scale);

    placement.rotation = if wants_random_rotation_y(definition) {
        Quat::from_rotation_y(compute_yaw(&mut rng))
    } else {
        Quat::IDENTITY
    };

    let chunk_size = layout.chunk_size_units();
    let margin = definition.placement_radius_meters.max(1.0);
    let local = placement.position.local.0;
    let (x, z) = apply_micro_jitter(
        local.x,
        local.z,
        margin,
        chunk_size,
        definition.placement_radius_meters,
        &mut rng,
    );
    placement.position.local.0.x = x;
    placement.position.local.0.z = z;

    true
}

/// Apply variation to all placements that resolve in `catalog`.
pub fn apply_placement_variation_batch(
    placements: &mut [FinalizedDoodadPlacement],
    catalog: &DoodadCatalog,
    layout: ChunkLayout,
) -> PlacementVariationSummary {
    let mut summary = PlacementVariationSummary::default();

    for placement in placements.iter_mut() {
        let Some(definition) = catalog.get(&placement.definition_id) else {
            continue;
        };

        if !apply_placement_variation(placement, definition, layout) {
            continue;
        }

        summary.placements_varied += 1;
        *summary.counts_by_kind.entry(definition.kind).or_insert(0) += 1;

        let scale = placement.scale.x;
        summary.scale_min = Some(summary.scale_min.map_or(scale, |m| m.min(scale)));
        summary.scale_max = Some(summary.scale_max.map_or(scale, |m| m.max(scale)));

        if wants_random_rotation_y(definition) {
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

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn tree_definition(min: f32, max: f32, random_y: bool) -> DoodadDefinition {
        let mut definition = DoodadDefinition::new(
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
        );
        if random_y {
            definition
                .placement_tags
                .push("random_rotation_y".to_string());
        }
        definition
    }

    fn placement_at(seed: u64, x: f32, z: f32) -> FinalizedDoodadPlacement {
        FinalizedDoodadPlacement {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed },
            position: WorldPosition::new(
                ChunkCoord::new(2, 3),
                LocalPosition::new(Vec3::new(x, 0.0, z)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    #[test]
    fn scale_variation_is_deterministic_per_seed() {
        let definition = tree_definition(0.8, 1.2, false);
        let mut a = placement_at(99, 64.0, 96.0);
        let mut b = placement_at(99, 64.0, 96.0);
        apply_placement_variation(&mut a, &definition, layout());
        apply_placement_variation(&mut b, &definition, layout());
        assert_eq!(a.scale, b.scale);
        assert!(a.scale.x >= 0.8 && a.scale.x <= 1.2);
    }

    #[test]
    fn rotation_variation_is_deterministic_per_seed() {
        let definition = tree_definition(1.0, 1.0, true);
        let mut a = placement_at(7, 128.0, 128.0);
        let mut b = placement_at(7, 128.0, 128.0);
        apply_placement_variation(&mut a, &definition, layout());
        apply_placement_variation(&mut b, &definition, layout());
        assert_eq!(a.rotation, b.rotation);
        assert_ne!(a.rotation, Quat::IDENTITY);
    }

    #[test]
    fn rotation_disabled_uses_identity() {
        let definition = tree_definition(1.0, 1.0, false);
        let mut placement = placement_at(5, 128.0, 128.0);
        apply_placement_variation(&mut placement, &definition, layout());
        assert_eq!(placement.rotation, Quat::IDENTITY);
    }

    #[test]
    fn different_seeds_produce_different_scale() {
        let definition = tree_definition(0.5, 1.5, false);
        let mut a = placement_at(1, 80.0, 80.0);
        let mut b = placement_at(2, 80.0, 80.0);
        apply_placement_variation(&mut a, &definition, layout());
        apply_placement_variation(&mut b, &definition, layout());
        assert_ne!(a.scale, b.scale);
    }

    #[test]
    fn different_chunks_produce_different_variation() {
        let definition = tree_definition(0.8, 1.2, true);
        let mut a = placement_at(42, 100.0, 100.0);
        let mut b = placement_at(42, 100.0, 100.0);
        b.position.chunk = ChunkCoord::new(9, 9);
        apply_placement_variation(&mut a, &definition, layout());
        apply_placement_variation(&mut b, &definition, layout());
        assert_ne!(a.rotation, b.rotation);
    }

    #[test]
    fn micro_jitter_stays_inside_chunk_margin() {
        let definition = tree_definition(1.0, 1.0, false);
        let margin = definition.placement_radius_meters.max(1.0);
        let chunk_size = layout().chunk_size_units();
        let mut placement = placement_at(11, margin, margin);
        apply_placement_variation(&mut placement, &definition, layout());
        let local = placement.position.local.0;
        assert!(local.x >= margin && local.x <= chunk_size - margin);
        assert!(local.z >= margin && local.z <= chunk_size - margin);
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
        let summary = apply_placement_variation_batch(&mut placements, &catalog, layout());
        assert_eq!(summary.placements_varied, 2);
        assert_eq!(summary.rotation_randomized, 1);
        assert_eq!(summary.rotation_identity, 1);
        assert_eq!(summary.counts_by_kind.get(&DoodadKind::Tree), Some(&1));
        assert_eq!(summary.counts_by_kind.get(&DoodadKind::Rock), Some(&1));
    }
}
