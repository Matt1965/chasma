//! Circle overlap tests for doodad movement obstacles (ADR-031, REVIEW-B6).

use bevy::prelude::*;

use crate::world::{
    ChunkCoord, ChunkId, DoodadCatalog, DoodadId, DoodadKind, DoodadRecord, WorldData,
    WorldPosition, default_blocks_movement,
};

use super::error::ObstacleQueryError;

/// Result of an obstacle overlap query at a world position.
#[derive(Debug, Clone, PartialEq)]
pub struct ObstacleQueryResult {
    pub blocked: bool,
    pub blocking_doodad: Option<DoodadId>,
    pub error: Option<ObstacleQueryError>,
}

/// Conservative movement block radius when catalog definition is missing (meters).
fn conservative_block_radius_for_kind(kind: DoodadKind) -> f32 {
    match kind {
        DoodadKind::Tree => 1.0,
        DoodadKind::Rock => 2.5,
        DoodadKind::Ruin => 4.0,
        DoodadKind::ResourceNode => 4.0,
        DoodadKind::Bush => 0.0,
    }
}

fn circle_overlap_blocked(center_a: Vec2, center_b: Vec2, radius_a: f32, radius_b: f32) -> bool {
    // Inclusive boundary: touching combined radius counts as blocked (REVIEW-B6).
    center_a.distance(center_b) <= radius_a + radius_b
}

fn blocking_params_for_record(
    record: &DoodadRecord,
    catalog: &DoodadCatalog,
) -> Result<(bool, f32), ObstacleQueryError> {
    if let Some(definition) = catalog.get(&record.definition_id) {
        return Ok((definition.blocks_movement, definition.block_radius_meters));
    }

    if !default_blocks_movement(record.kind) {
        return Ok((false, 0.0));
    }

    let radius = conservative_block_radius_for_kind(record.kind);
    if !(radius > 0.0) || !radius.is_finite() {
        return Err(ObstacleQueryError::InvalidBlockingRadius {
            radius_meters: radius,
        });
    }

    Err(ObstacleQueryError::MissingDoodadDefinition {
        definition_id: record.definition_id.clone(),
    })
}

/// Authoritative obstacle query with structured diagnostics.
pub fn query_obstacle_at_position(
    world: &WorldData,
    catalog: &DoodadCatalog,
    position: WorldPosition,
    radius_meters: f32,
) -> ObstacleQueryResult {
    if !(radius_meters >= 0.0) || !radius_meters.is_finite() {
        return ObstacleQueryResult {
            blocked: true,
            blocking_doodad: None,
            error: Some(ObstacleQueryError::InvalidBlockingRadius { radius_meters }),
        };
    }

    let layout = world.layout();
    let center = position.to_global(layout);
    let center_xz = Vec2::new(center.x, center.z);

    let mut chunks: Vec<ChunkCoord> = Vec::with_capacity(9);
    for dz in -1..=1 {
        for dx in -1..=1 {
            chunks.push(ChunkCoord::new(
                position.chunk.x + dx,
                position.chunk.z + dz,
            ));
        }
    }
    chunks.sort_by_key(|coord| (coord.x, coord.z));

    let mut first_error: Option<ObstacleQueryError> = None;

    for chunk_coord in chunks {
        let chunk_id = ChunkId::new(chunk_coord);
        let Some(store) = world.doodads_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            let (blocks, block_radius) = match blocking_params_for_record(record, catalog) {
                Ok(params) => params,
                Err(err) => {
                    first_error.get_or_insert(err.clone());
                    if matches!(err, ObstacleQueryError::MissingDoodadDefinition { .. }) {
                        let radius = conservative_block_radius_for_kind(record.kind);
                        if default_blocks_movement(record.kind)
                            && circle_overlap_blocked(
                                center_xz,
                                Vec2::new(
                                    record.placement.position.to_global(layout).x,
                                    record.placement.position.to_global(layout).z,
                                ),
                                radius_meters,
                                radius,
                            )
                        {
                            return ObstacleQueryResult {
                                blocked: true,
                                blocking_doodad: Some(record.id),
                                error: Some(err),
                            };
                        }
                    }
                    continue;
                }
            };

            if !blocks {
                continue;
            }
            if !(block_radius >= 0.0) || !block_radius.is_finite() {
                first_error.get_or_insert(ObstacleQueryError::InvalidBlockingRadius {
                    radius_meters: block_radius,
                });
                return ObstacleQueryResult {
                    blocked: true,
                    blocking_doodad: Some(record.id),
                    error: first_error,
                };
            }

            let doodad_global = record.placement.position.to_global(layout);
            let doodad_xz = Vec2::new(doodad_global.x, doodad_global.z);
            if circle_overlap_blocked(center_xz, doodad_xz, radius_meters, block_radius) {
                return ObstacleQueryResult {
                    blocked: true,
                    blocking_doodad: Some(record.id),
                    error: first_error,
                };
            }
        }
    }

    ObstacleQueryResult {
        blocked: false,
        blocking_doodad: None,
        error: first_error,
    }
}

/// Return `true` when a unit footprint at `position` overlaps a blocking doodad.
///
/// Fail-closed: any query error is treated as blocked even when overlap cannot be
/// confirmed (REVIEW-B6).
pub fn is_position_blocked_by_doodads(
    world: &WorldData,
    catalog: &DoodadCatalog,
    position: WorldPosition,
    radius_meters: f32,
) -> bool {
    let result = query_obstacle_at_position(world, catalog, position, radius_meters);
    result.blocked || result.error.is_some()
}

/// The first blocking doodad id overlapping `position`, if any (deterministic order).
pub fn blocking_doodad_at_position(
    world: &WorldData,
    catalog: &DoodadCatalog,
    position: WorldPosition,
    radius_meters: f32,
) -> Option<DoodadId> {
    let result = query_obstacle_at_position(world, catalog, position, radius_meters);
    if result.blocked {
        result.blocking_doodad
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId,
        DoodadPlacementOverrides, DoodadSource, Heightfield, LocalPosition, create_doodad,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(chunk_x: i32, chunk_z: i32, x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(chunk_x, chunk_z),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn insert_flat(world: &mut WorldData, x: i32, z: i32) {
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn place_tree(world: &mut WorldData, catalog: &DoodadCatalog, position: WorldPosition) {
        create_doodad(
            catalog,
            world,
            &DoodadDefinitionId::new("tree_oak"),
            position,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
    }

    #[test]
    fn tree_definition_blocks_by_default() {
        let catalog = DoodadCatalog::default();
        let tree = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
        assert!(tree.blocks_movement);
        assert_eq!(tree.block_radius_meters, 1.0);
        assert_eq!(tree.placement_radius_meters, 4.0);

        let bush = catalog.get(&DoodadDefinitionId::new("bush_scrub")).unwrap();
        assert!(!bush.blocks_movement);
    }

    #[test]
    fn blocks_when_footprint_overlaps() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(0, 0, 50.0, 50.0));

        assert!(is_position_blocked_by_doodads(
            &world,
            &catalog,
            pos(0, 0, 50.0, 50.0),
            0.5,
        ));
    }

    #[test]
    fn allows_when_outside_combined_radius() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(0, 0, 50.0, 50.0));
        let tree = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();

        let unit_pos = pos(0, 0, 50.0 + tree.block_radius_meters + 1.0, 50.0);
        assert!(!is_position_blocked_by_doodads(
            &world, &catalog, unit_pos, 0.5,
        ));
    }

    #[test]
    fn exact_combined_radius_counts_as_blocked() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(0, 0, 50.0, 50.0));
        let tree = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
        let unit_radius = 0.5;
        let edge = tree.block_radius_meters + unit_radius;
        let unit_pos = pos(0, 0, 50.0 + edge, 50.0);
        assert!(is_position_blocked_by_doodads(
            &world,
            &catalog,
            unit_pos,
            unit_radius,
        ));
    }

    #[test]
    fn neighbor_chunk_obstacle_blocks_near_border() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(1, 0, 0.0, 128.0));

        assert!(is_position_blocked_by_doodads(
            &world,
            &catalog,
            pos(0, 0, 255.0, 128.0),
            0.5,
        ));
    }

    #[test]
    fn non_blocking_doodad_does_not_block() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("bush_scrub"),
            pos(0, 0, 10.0, 10.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        assert!(!is_position_blocked_by_doodads(
            &world,
            &catalog,
            pos(0, 0, 10.0, 10.0),
            0.5,
        ));
    }

    #[test]
    fn missing_definition_fails_closed_for_blocking_kind() {
        use crate::world::{DoodadId, DoodadKind, DoodadPlacement, DoodadRecord, DoodadSource};

        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let position = pos(0, 0, 20.0, 20.0);
        let record = DoodadRecord::new(
            DoodadId::new(99),
            DoodadDefinitionId::new("missing_tree_def"),
            DoodadKind::Tree,
            DoodadPlacement::new(position, Quat::IDENTITY, Vec3::ONE),
            DoodadSource::Authored,
        );
        world
            .insert_doodad(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();

        let result = query_obstacle_at_position(&world, &catalog, position, 0.5);
        assert!(result.blocked);
        assert!(matches!(
            result.error,
            Some(ObstacleQueryError::MissingDoodadDefinition { .. })
        ));
    }

    #[test]
    fn missing_definition_fail_closed_even_without_overlap() {
        use crate::world::{DoodadId, DoodadKind, DoodadPlacement, DoodadRecord, DoodadSource};

        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let tree_position = pos(0, 0, 20.0, 20.0);
        let far_position = pos(0, 0, 200.0, 200.0);
        let record = DoodadRecord::new(
            DoodadId::new(99),
            DoodadDefinitionId::new("missing_tree_def"),
            DoodadKind::Tree,
            DoodadPlacement::new(tree_position, Quat::IDENTITY, Vec3::ONE),
            DoodadSource::Authored,
        );
        world
            .insert_doodad(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();

        let result = query_obstacle_at_position(&world, &catalog, far_position, 0.5);
        assert!(!result.blocked);
        assert!(matches!(
            result.error,
            Some(ObstacleQueryError::MissingDoodadDefinition { .. })
        ));
        assert!(is_position_blocked_by_doodads(
            &world,
            &catalog,
            far_position,
            0.5,
        ));
    }

    #[test]
    fn uses_world_data_not_render_entities() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        place_tree(&mut world, &catalog, pos(0, 0, 20.0, 20.0));
        assert!(is_position_blocked_by_doodads(
            &world,
            &catalog,
            pos(0, 0, 20.0, 20.0),
            0.5,
        ));
    }
}
