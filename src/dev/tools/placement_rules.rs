//! Dev placement validation — reuses authoritative world queries (ADR-044).

use bevy::prelude::*;

use crate::world::{
    ground_world_position, is_position_blocked_by_doodads, is_position_slope_walkable,
    DoodadCatalog, UnitCatalog, WorldData, WorldPosition,
};

use super::super::dev_mode::DefinitionId;

/// Rules applied before committing dev placements.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct PlacementRules {
    pub snap_to_terrain: bool,
    /// Reserved hook — when true, reject positions overlapping blocking doodads.
    pub avoid_doodads: bool,
    pub min_distance_between_entities: f32,
    pub enforce_slope: bool,
    /// When true and biome mask is loaded, doodad definitions must allow sampled biome.
    pub enforce_biome: bool,
}

impl Default for PlacementRules {
    fn default() -> Self {
        Self {
            snap_to_terrain: true,
            avoid_doodads: true,
            min_distance_between_entities: 1.5,
            enforce_slope: true,
            enforce_biome: true,
        }
    }
}

/// Why a candidate position was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementRejectReason {
    TerrainUnavailable,
    SlopeTooSteep,
    BlockedByDoodad,
    BiomeDisallowed,
    TooCloseToPeer,
}

/// Outcome of validating one placement candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlacementValidation {
    Accepted(WorldPosition),
    Rejected(PlacementRejectReason),
}

/// Context for placement validation (read-only on [`WorldData`]).
pub struct PlacementValidateContext<'a> {
    pub world: &'a WorldData,
    pub unit_catalog: &'a UnitCatalog,
    pub doodad_catalog: &'a DoodadCatalog,
    pub definition: &'a DefinitionId,
    pub rules: &'a PlacementRules,
}

impl PlacementValidation {
    pub fn position(self) -> Option<WorldPosition> {
        match self {
            Self::Accepted(position) => Some(position),
            Self::Rejected(_) => None,
        }
    }
}

/// Validate and optionally snap one candidate; `accepted_peers` holds prior batch accepts.
pub fn validate_placement(
    ctx: &PlacementValidateContext<'_>,
    candidate: WorldPosition,
    accepted_peers: &[WorldPosition],
) -> PlacementValidation {
    let position = if ctx.rules.snap_to_terrain {
        match ground_world_position(ctx.world, candidate) {
            Some(grounded) => grounded,
            None => return PlacementValidation::Rejected(PlacementRejectReason::TerrainUnavailable),
        }
    } else {
        candidate
    };

    if ctx.rules.enforce_slope {
        let max_slope = max_slope_for_definition(ctx);
        if !is_position_slope_walkable(ctx.world, position, max_slope) {
            return PlacementValidation::Rejected(PlacementRejectReason::SlopeTooSteep);
        }
    }

    if ctx.rules.avoid_doodads {
        let agent_radius = agent_radius_for_definition(ctx);
        if is_position_blocked_by_doodads(
            ctx.world,
            ctx.doodad_catalog,
            position,
            agent_radius,
        ) {
            return PlacementValidation::Rejected(PlacementRejectReason::BlockedByDoodad);
        }
    }

    if ctx.rules.enforce_biome {
        if let DefinitionId::Doodad(definition_id) = ctx.definition {
            if let Some(definition) = ctx.doodad_catalog.get(definition_id) {
                if !definition.allowed_biomes.is_empty() {
                    if let Some(sample) = ctx.world.biome_at(position) {
                        if !definition.allows_biome(sample.biome) {
                            return PlacementValidation::Rejected(
                                PlacementRejectReason::BiomeDisallowed,
                            );
                        }
                    }
                }
            }
        }
    }

    let min_dist = ctx.rules.min_distance_between_entities;
    if min_dist > 0.0 {
        for peer in accepted_peers {
            if xz_distance(ctx.world, position, *peer) < min_dist {
                return PlacementValidation::Rejected(PlacementRejectReason::TooCloseToPeer);
            }
        }
    }

    PlacementValidation::Accepted(position)
}

fn max_slope_for_definition(ctx: &PlacementValidateContext<'_>) -> f32 {
    match ctx.definition {
        DefinitionId::Unit(id) => ctx
            .unit_catalog
            .get(id)
            .map(|def| def.max_slope_degrees)
            .unwrap_or(45.0),
        DefinitionId::Doodad(id) => ctx
            .doodad_catalog
            .get(id)
            .and_then(|def| def.max_slope_degrees)
            .unwrap_or(45.0),
    }
}

fn agent_radius_for_definition(ctx: &PlacementValidateContext<'_>) -> f32 {
    match ctx.definition {
        DefinitionId::Unit(id) => ctx
            .unit_catalog
            .get(id)
            .map(|def| def.collision_radius_meters)
            .unwrap_or(0.5),
        DefinitionId::Doodad(id) => ctx
            .doodad_catalog
            .get(id)
            .map(|def| def.placement_radius_meters.max(def.block_radius_meters))
            .unwrap_or(0.5),
    }
}

fn xz_distance(world: &WorldData, a: WorldPosition, b: WorldPosition) -> f32 {
    let layout = world.layout();
    let ga = a.to_global(layout);
    let gb = b.to_global(layout);
    Vec2::new(ga.x - gb.x, ga.z - gb.z).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        estimate_slope_degrees, is_position_slope_walkable, ChunkCoord, ChunkData, ChunkId,
        ChunkLayout, Heightfield, LocalPosition, UnitDefinition, UnitDefinitionId, UnitRenderKey,
    };

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn terrain_snapping_adjusts_y() {
        let world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let definition = DefinitionId::Unit(UnitDefinitionId::new("wolf"));
        let rules = PlacementRules::default();
        let ctx = PlacementValidateContext {
            world: &world,
            unit_catalog: &unit_catalog,
            doodad_catalog: &doodad_catalog,
            definition: &definition,
            rules: &rules,
        };
        let candidate = pos(10.0, 10.0);
        let result = validate_placement(&ctx, candidate, &[]);
        assert!(matches!(result, PlacementValidation::Accepted(_)));
    }

    #[test]
    fn min_distance_rejects_close_peers() {
        let world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let definition = DefinitionId::Unit(UnitDefinitionId::new("wolf"));
        let rules = PlacementRules {
            min_distance_between_entities: 4.0,
            ..PlacementRules::default()
        };
        let ctx = PlacementValidateContext {
            world: &world,
            unit_catalog: &unit_catalog,
            doodad_catalog: &doodad_catalog,
            definition: &definition,
            rules: &rules,
        };
        let first = validate_placement(&ctx, pos(20.0, 20.0), &[]);
        let accepted = first.position().unwrap();
        let second = validate_placement(&ctx, pos(21.0, 20.0), &[accepted]);
        assert!(matches!(
            second,
            PlacementValidation::Rejected(PlacementRejectReason::TooCloseToPeer)
        ));
    }

    fn steep_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let mut samples = Vec::new();
        for _row in 0..3 {
            for col in 0..3 {
                samples.push(col as f32 * 40.0);
            }
        }
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn strict_slope_catalog() -> UnitCatalog {
        UnitCatalog::from_definitions(vec![UnitDefinition::new(
            UnitDefinitionId::new("strict"),
            "Strict",
            "Test",
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1.0,
            "Test",
            1.0,
            0.5,
            5.0,
            true,
            UnitRenderKey::reserved("strict"),
        )])
        .unwrap()
    }

    #[test]
    fn invalid_slope_positions_are_rejected() {
        let world = steep_world();
        let unit_catalog = strict_slope_catalog();
        let doodad_catalog = DoodadCatalog::default();
        let definition = DefinitionId::Unit(UnitDefinitionId::new("strict"));
        let rules = PlacementRules {
            enforce_slope: true,
            ..PlacementRules::default()
        };
        let ctx = PlacementValidateContext {
            world: &world,
            unit_catalog: &unit_catalog,
            doodad_catalog: &doodad_catalog,
            definition: &definition,
            rules: &rules,
        };
        let candidate = pos(64.0, 64.0);
        let chunk = world
            .get(ChunkId::new(ChunkCoord::new(0, 0)))
            .unwrap();
        let slope = estimate_slope_degrees(&chunk.heightfield, 64.0, 64.0).unwrap();
        assert!(slope > 5.0);
        assert!(!is_position_slope_walkable(&world, candidate, 5.0));
        let result = validate_placement(&ctx, candidate, &[]);
        assert!(matches!(
            result,
            PlacementValidation::Rejected(PlacementRejectReason::SlopeTooSteep)
        ));
    }
}
