//! Dev placement validation — reuses authoritative world queries (ADR-044).

use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, DoodadCatalog, FootprintCatalog, SlopeWalkability, UnitCatalog, WorldData,
    WorldPosition, classify_slope_walkability, ground_world_position,
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
}

impl Default for PlacementRules {
    fn default() -> Self {
        Self {
            snap_to_terrain: true,
            avoid_doodads: true,
            min_distance_between_entities: 1.5,
            enforce_slope: true,
        }
    }
}

/// Why a candidate position was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementRejectReason {
    TerrainUnavailable,
    SlopeUnavailable,
    SlopeTooSteep,
    BlockedByDoodad,
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
    pub building_catalog: &'a BuildingCatalog,
    pub footprint_catalog: &'a FootprintCatalog,
    pub definition: &'a DefinitionId,
    pub rules: &'a PlacementRules,
}

impl PlacementValidation {
    #[cfg(test)]
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
            None => {
                return PlacementValidation::Rejected(PlacementRejectReason::TerrainUnavailable);
            }
        }
    } else {
        candidate
    };

    if ctx.rules.enforce_slope {
        let max_slope = max_slope_for_definition(ctx);
        match classify_slope_walkability(ctx.world, position, max_slope) {
            SlopeWalkability::Walkable => {}
            SlopeWalkability::TooSteep => {
                return PlacementValidation::Rejected(PlacementRejectReason::SlopeTooSteep);
            }
            SlopeWalkability::Unavailable => {
                return PlacementValidation::Rejected(PlacementRejectReason::SlopeUnavailable);
            }
        }
    }

    if ctx.rules.avoid_doodads {
        let agent_radius = agent_radius_for_definition(ctx);
        if crate::world::is_position_blocked_for_agent(
            ctx.world,
            crate::world::PassabilityCatalogs {
                doodad: ctx.doodad_catalog,
                building: ctx.building_catalog,
                footprint: ctx.footprint_catalog,
            },
            position,
            agent_radius,
            max_slope_for_definition(ctx),
        ) {
            return PlacementValidation::Rejected(PlacementRejectReason::BlockedByDoodad);
        }
    }

    // Dev-authored doodad placement ignores biome restrictions from the catalog.

    let min_dist = ctx.rules.min_distance_between_entities;
    if min_dist > 0.0 {
        for peer in accepted_peers {
            if xz_distance(ctx.world, position, *peer) < min_dist {
                return PlacementValidation::Rejected(PlacementRejectReason::TooCloseToPeer);
            }
        }
    }

    PlacementValidation::Accepted(finalize_dev_placement_position(ctx, position))
}

fn finalize_dev_placement_position(
    ctx: &PlacementValidateContext<'_>,
    position: WorldPosition,
) -> WorldPosition {
    if matches!(ctx.definition, DefinitionId::Building(_)) {
        return crate::world::ground_and_quantize_building_anchor(ctx.world, position)
            .unwrap_or(position);
    }
    position
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
        DefinitionId::Building(id) => ctx
            .building_catalog
            .get(id)
            .map(|def| def.max_slope_degrees)
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
            .map(crate::world::doodad_definition_placement_radius_meters)
            .unwrap_or(0.5),
        DefinitionId::Building(id) => ctx
            .building_catalog
            .get(id)
            .map(|def| match def.footprint {
                crate::world::FootprintSpec::Circle { radius_meters } => radius_meters,
                crate::world::FootprintSpec::Rectangle {
                    width_meters,
                    depth_meters,
                } => width_meters.max(depth_meters) * 0.5,
                crate::world::FootprintSpec::MeshDerived => 1.5,
            })
            .unwrap_or(1.0),
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
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, SlopeWalkability,
        UnitDefinition, UnitDefinitionId, UnitRenderKey, classify_slope_walkability,
        estimate_slope_degrees,
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
            building_catalog: &BuildingCatalog::default(),
            footprint_catalog: &FootprintCatalog::default(),
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
            building_catalog: &BuildingCatalog::default(),
            footprint_catalog: &FootprintCatalog::default(),
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
            1,
            1.0,
            "Test",
            1.0,
            0.5,
            5.0,
            crate::world::WeaponDefinitionId::new("weapon_fists"),
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
            building_catalog: &BuildingCatalog::default(),
            footprint_catalog: &FootprintCatalog::default(),
            definition: &definition,
            rules: &rules,
        };
        let candidate = pos(64.0, 64.0);
        let chunk = world.get(ChunkId::new(ChunkCoord::new(0, 0))).unwrap();
        let slope = estimate_slope_degrees(&chunk.heightfield, 64.0, 64.0).unwrap();
        assert!(slope > 5.0);
        assert_eq!(
            classify_slope_walkability(&world, candidate, 5.0),
            SlopeWalkability::TooSteep
        );
        let result = validate_placement(&ctx, candidate, &[]);
        assert!(matches!(
            result,
            PlacementValidation::Rejected(PlacementRejectReason::SlopeTooSteep)
        ));
    }

    #[test]
    fn missing_terrain_reports_slope_unavailable_when_not_snapping() {
        let world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let definition = DefinitionId::Unit(UnitDefinitionId::new("wolf"));
        let rules = PlacementRules {
            snap_to_terrain: false,
            enforce_slope: true,
            ..PlacementRules::default()
        };
        let ctx = PlacementValidateContext {
            world: &world,
            unit_catalog: &unit_catalog,
            doodad_catalog: &doodad_catalog,
            building_catalog: &BuildingCatalog::default(),
            footprint_catalog: &FootprintCatalog::default(),
            definition: &definition,
            rules: &rules,
        };
        let result = validate_placement(&ctx, pos(64.0, 64.0), &[]);
        assert!(matches!(
            result,
            PlacementValidation::Rejected(PlacementRejectReason::SlopeUnavailable)
        ));
    }
}
