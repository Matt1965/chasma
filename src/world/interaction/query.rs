//! Unified world interaction query (ADR-042 U6).
//!
//! Composes terrain heightfield queries and doodad obstacle queries without
//! merging those systems.

use bevy::prelude::*;

use crate::world::obstacle::blocking_doodad_at_position;
use crate::world::{
    ground_world_position, is_position_slope_walkable, ChunkCoord, ChunkId, DoodadCatalog,
    DoodadDefinition, DoodadId, DoodadKind, DoodadRecord, UnitCatalog, WorldData, WorldPosition,
};

use super::types::{
    InteractionMetadata, InteractionResult, InteractionTargetRef, InteractionType,
};

/// Default query radius for “under cursor” classification (meters).
pub const DEFAULT_INTERACTION_QUERY_RADIUS_METERS: f32 = 2.5;

/// Default agent footprint radius when classifying blocked areas.
pub const DEFAULT_INTERACTION_AGENT_RADIUS_METERS: f32 = 0.5;

/// Default max slope for walkable move targets (degrees).
pub const DEFAULT_INTERACTION_MAX_SLOPE_DEGREES: f32 = 40.0;

/// Inputs for [`query_world_interaction`].
#[derive(Debug, Clone, Copy)]
pub struct InteractionQueryContext<'a> {
    pub world: &'a WorldData,
    pub doodad_catalog: &'a DoodadCatalog,
    pub unit_catalog: &'a UnitCatalog,
    pub query_radius_meters: f32,
    pub agent_radius_meters: f32,
    pub max_slope_degrees: f32,
}

impl<'a> InteractionQueryContext<'a> {
    pub fn new(
        world: &'a WorldData,
        doodad_catalog: &'a DoodadCatalog,
        unit_catalog: &'a UnitCatalog,
    ) -> Self {
        Self {
            world,
            doodad_catalog,
            unit_catalog,
            query_radius_meters: DEFAULT_INTERACTION_QUERY_RADIUS_METERS,
            agent_radius_meters: DEFAULT_INTERACTION_AGENT_RADIUS_METERS,
            max_slope_degrees: DEFAULT_INTERACTION_MAX_SLOPE_DEGREES,
        }
    }
}

/// Classify the world at `position` within `query_radius_meters`.
///
/// Read-only — never mutates [`WorldData`].
pub fn query_world_interaction(
    ctx: &InteractionQueryContext<'_>,
    position: WorldPosition,
) -> Option<InteractionResult> {
    let Some(grounded) = ground_world_position(ctx.world, position) else {
        return None;
    };

    if let Some((record, definition)) =
        nearest_doodad_in_radius(ctx, grounded, ctx.query_radius_meters)
    {
        return Some(classify_doodad_hit(grounded, record, definition));
    }

    if blocking_doodad_at_position(
        ctx.world,
        ctx.doodad_catalog,
        grounded,
        ctx.agent_radius_meters,
    )
    .is_some()
    {
        return Some(InteractionResult {
            interaction_type: InteractionType::BlockedArea,
            position: grounded,
            metadata: InteractionMetadata {
                label: "Blocked".to_string(),
                doodad_kind: None,
                blocks_movement: true,
            },
            valid: true,
            target: InteractionTargetRef::Terrain(grounded),
        });
    }

    let walkable = is_position_slope_walkable(ctx.world, grounded, ctx.max_slope_degrees);
    if !walkable {
        return Some(InteractionResult {
            interaction_type: InteractionType::BlockedArea,
            position: grounded,
            metadata: InteractionMetadata {
                label: "Unwalkable terrain".to_string(),
                doodad_kind: None,
                blocks_movement: true,
            },
            valid: true,
            target: InteractionTargetRef::Terrain(grounded),
        });
    }

    Some(InteractionResult {
        interaction_type: InteractionType::MoveTarget,
        position: grounded,
        metadata: InteractionMetadata {
            label: "Move".to_string(),
            doodad_kind: None,
            blocks_movement: false,
        },
        valid: true,
        target: InteractionTargetRef::Terrain(grounded),
    })
}

fn classify_doodad_hit(
    grounded: WorldPosition,
    record: &DoodadRecord,
    definition: &DoodadDefinition,
) -> InteractionResult {
    let interaction_type = if definition.kind == DoodadKind::ResourceNode {
        InteractionType::ResourceNode
    } else if definition.blocks_movement {
        InteractionType::BlockedArea
    } else {
        InteractionType::InteractableObject
    };

    InteractionResult {
        interaction_type,
        position: grounded,
        metadata: InteractionMetadata {
            label: definition.display_name.clone(),
            doodad_kind: Some(definition.kind),
            blocks_movement: definition.blocks_movement,
        },
        valid: true,
        target: InteractionTargetRef::Doodad(record.id),
    }
}

fn nearest_doodad_in_radius<'a>(
    ctx: &'a InteractionQueryContext<'_>,
    position: WorldPosition,
    radius_meters: f32,
) -> Option<(&'a DoodadRecord, &'a DoodadDefinition)> {
    let layout = ctx.world.layout();
    let center = position.to_global(layout);
    let center_xz = Vec2::new(center.x, center.z);

    let mut best: Option<(f32, DoodadId)> = None;
    let mut best_record: Option<&'a DoodadRecord> = None;
    let mut best_definition: Option<&'a DoodadDefinition> = None;

    let mut chunks: Vec<ChunkCoord> = Vec::with_capacity(9);
    for dz in -1..=1 {
        for dx in -1..=1 {
            chunks.push(ChunkCoord::new(position.chunk.x + dx, position.chunk.z + dz));
        }
    }
    chunks.sort_by_key(|coord| (coord.x, coord.z));

    for chunk_coord in chunks {
        let chunk_id = ChunkId::new(chunk_coord);
        let Some(store) = ctx.world.doodads_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            let Some(definition) = ctx.doodad_catalog.get(&record.definition_id) else {
                continue;
            };
            let doodad_global = record.placement.position.to_global(layout);
            let doodad_xz = Vec2::new(doodad_global.x, doodad_global.z);
            let reach = radius_meters + definition.placement_radius_meters.max(0.5);
            let distance = center_xz.distance(doodad_xz);
            if distance > reach {
                continue;
            }
            let replace = match &best {
                None => true,
                Some((best_dist, best_id)) => {
                    distance < *best_dist - 1e-4
                        || ((distance - *best_dist).abs() <= 1e-4 && record.id.raw() < best_id.raw())
                }
            };
            if replace {
                best = Some((distance, record.id));
                best_record = Some(record);
                best_definition = Some(definition);
            }
        }
    }

    match (best_record, best_definition) {
        (Some(record), Some(definition)) => Some((record, definition)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_doodad, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId,
        DoodadPlacementOverrides, DoodadSource, Heightfield, LocalPosition,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn ctx<'a>(
        world: &'a WorldData,
        catalog: &'a DoodadCatalog,
        unit_catalog: &'a UnitCatalog,
    ) -> InteractionQueryContext<'a> {
        InteractionQueryContext::new(world, catalog, unit_catalog)
    }

    #[test]
    fn terrain_click_returns_move_target() {
        let world = flat_world();
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let result =
            query_world_interaction(&ctx(&world, &catalog, &unit_catalog), pos(64.0, 64.0))
                .unwrap();
        assert_eq!(result.interaction_type, InteractionType::MoveTarget);
        assert!(result.valid);
    }

    #[test]
    fn blocking_doodad_returns_blocked_area() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let mut world = flat_world();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(50.0, 50.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        let result =
            query_world_interaction(&ctx(&world, &catalog, &unit_catalog), pos(50.0, 50.0))
                .unwrap();
        assert_eq!(result.interaction_type, InteractionType::BlockedArea);
        assert!(matches!(result.target, InteractionTargetRef::Doodad(_)));
    }

    #[test]
    fn non_blocking_doodad_returns_interactable_object() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let mut world = flat_world();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("bush_scrub"),
            pos(30.0, 30.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        let result =
            query_world_interaction(&ctx(&world, &catalog, &unit_catalog), pos(30.0, 30.0))
                .unwrap();
        assert_eq!(result.interaction_type, InteractionType::InteractableObject);
    }

    #[test]
    fn resource_node_returns_resource_node_type() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let mut world = flat_world();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("resource_node_iron"),
            pos(70.0, 70.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        let result =
            query_world_interaction(&ctx(&world, &catalog, &unit_catalog), pos(70.0, 70.0))
                .unwrap();
        assert_eq!(result.interaction_type, InteractionType::ResourceNode);
    }

    #[test]
    fn invalid_position_without_terrain_returns_none() {
        let world = WorldData::new(layout());
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        assert!(
            query_world_interaction(&ctx(&world, &catalog, &unit_catalog), pos(1.0, 1.0)).is_none()
        );
    }

    #[test]
    fn query_layer_does_not_mutate_world_data() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let world = flat_world();
        let chunks_before = world.len();
        let _ = query_world_interaction(
            &InteractionQueryContext::new(&world, &catalog, &unit_catalog),
            pos(10.0, 10.0),
        );
        assert_eq!(world.len(), chunks_before);
    }

    #[test]
    fn classification_is_deterministic() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let world = flat_world();
        let a = query_world_interaction(&ctx(&world, &catalog, &unit_catalog), pos(12.0, 14.0));
        let b = query_world_interaction(&ctx(&world, &catalog, &unit_catalog), pos(12.0, 14.0));
        assert_eq!(a, b);
    }
}
