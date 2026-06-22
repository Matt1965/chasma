//! Authoritative straight-line unit movement (ADR-030 U5).
//!
//! Steps [`UnitRecord`] placement on the XZ plane toward a [`UnitState::Moving`]
//! target, grounding Y from resident heightfields. No pathfinding or collision.

use bevy::prelude::*;

use super::catalog::UnitCatalog;
use super::id::UnitId;
use super::state::UnitState;
use super::UnitInsertError;
use crate::world::{
    estimate_slope_degrees, ground_world_position, ChunkId, WorldData, WorldPosition,
};

/// Distance below which a unit snaps to its move target (meters).
const ARRIVAL_DISTANCE_METERS: f32 = 0.05;

/// Outcome of one movement step for a single unit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitMovementStepReport {
    pub moved: bool,
    pub arrived: bool,
}

/// Aggregated outcome of [`step_all_unit_movement`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BatchUnitMovementReport {
    pub moved: u32,
    pub arrived: u32,
    pub blocked_terrain_unavailable: u32,
    pub blocked_slope_unavailable: u32,
    pub blocked_slope_too_steep: u32,
    pub missing_definition: u32,
}

/// Why [`step_unit_movement`] could not complete a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitMovementError {
    UnitNotFound,
    DefinitionNotFound,
    TerrainUnavailable,
    SlopeUnavailable,
    SlopeTooSteep,
}

/// Advance one unit along a straight line toward its move target.
pub fn step_unit_movement(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    unit_id: UnitId,
    delta_seconds: f32,
) -> Result<UnitMovementStepReport, UnitMovementError> {
    let record = world
        .get_unit(unit_id)
        .ok_or(UnitMovementError::UnitNotFound)?;
    let definition_id = record.definition_id.clone();
    let UnitState::Moving { target } = record.state else {
        return Ok(UnitMovementStepReport::default());
    };
    let current_position = record.placement.position;

    let definition = catalog
        .get(&definition_id)
        .ok_or(UnitMovementError::DefinitionNotFound)?;

    let layout = world.layout();
    let current_global = current_position.to_global(layout);
    let target_global = target.to_global(layout);
    let mut to_target = target_global - current_global;
    to_target.y = 0.0;
    let distance = to_target.length();
    let step_distance = definition.move_speed_mps * delta_seconds;

    let destination_global = if distance <= step_distance.max(ARRIVAL_DISTANCE_METERS) {
        Vec3::new(target_global.x, current_global.y, target_global.z)
    } else {
        let direction = to_target / distance;
        current_global + direction * step_distance
    };

    let candidate = WorldPosition::from_global(destination_global, layout);
    let grounded = ground_world_position(world, candidate).ok_or(UnitMovementError::TerrainUnavailable)?;

    validate_slope_at_position(world, grounded, definition.max_slope_degrees)?;

    world
        .relocate_unit(unit_id, grounded)
        .map_err(|error| match error {
            UnitInsertError::UnitNotFound => UnitMovementError::UnitNotFound,
            UnitInsertError::ChunkPlacementMismatch => UnitMovementError::TerrainUnavailable,
        })?;

    let arrived = distance <= step_distance.max(ARRIVAL_DISTANCE_METERS);
    if arrived {
        world
            .set_unit_state(unit_id, UnitState::Idle)
            .map_err(|_| UnitMovementError::UnitNotFound)?;
    } else {
        world
            .set_unit_state(unit_id, UnitState::Moving { target })
            .map_err(|_| UnitMovementError::UnitNotFound)?;
    }

    Ok(UnitMovementStepReport {
        moved: true,
        arrived,
    })
}

/// Advance all units deterministically by [`UnitId`].
pub fn step_all_unit_movement(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    delta_seconds: f32,
) -> BatchUnitMovementReport {
    let mut report = BatchUnitMovementReport::default();
    for unit_id in world.sorted_unit_ids() {
        match step_unit_movement(world, catalog, unit_id, delta_seconds) {
            Ok(step) => {
                if step.moved {
                    report.moved += 1;
                }
                if step.arrived {
                    report.arrived += 1;
                }
            }
            Err(UnitMovementError::DefinitionNotFound) => report.missing_definition += 1,
            Err(UnitMovementError::TerrainUnavailable) => report.blocked_terrain_unavailable += 1,
            Err(UnitMovementError::SlopeUnavailable) => report.blocked_slope_unavailable += 1,
            Err(UnitMovementError::SlopeTooSteep) => report.blocked_slope_too_steep += 1,
            Err(UnitMovementError::UnitNotFound) => {}
        }
    }
    report
}

fn validate_slope_at_position(
    world: &WorldData,
    position: WorldPosition,
    max_slope_degrees: f32,
) -> Result<(), UnitMovementError> {
    let chunk_id = ChunkId::new(position.chunk);
    let data = world
        .get(chunk_id)
        .ok_or(UnitMovementError::TerrainUnavailable)?;
    let slope = estimate_slope_degrees(
        &data.heightfield,
        position.local.0.x,
        position.local.0.z,
    )
    .ok_or(UnitMovementError::SlopeUnavailable)?;
    if slope > max_slope_degrees {
        return Err(UnitMovementError::SlopeTooSteep);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{world_position_to_render_global, TerrainRenderAssets};
    use crate::units::{
        sync_unit_render_entities, UnitRenderIndex, UnitSceneAssets, UnitSyncOverrides,
    };
    use crate::world::{
        create_unit, issue_unit_order, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield,
        LocalPosition, UnitCatalog, UnitDefinition, UnitDefinitionId, UnitMetadata, UnitOrder,
        UnitPlacement, UnitRecord, UnitRenderKey, UnitSource, WorldConfig,
    };
    use bevy::asset::AssetPlugin;
    use bevy::prelude::{
        App, Assets, MinimalPlugins, Quat, Scene, StandardMaterial, Transform, Update, Vec3, World,
    };
    use std::collections::HashMap;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_chunk(height: f32) -> ChunkData {
        let heightfield = Heightfield::from_samples(3, 128.0, vec![height; 9]).unwrap();
        ChunkData::new(heightfield, Vec::new())
    }

    fn insert_flat(world: &mut WorldData, x: i32, z: i32, height: f32) -> ChunkId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        world.insert(chunk, flat_chunk(height));
        chunk
    }

    fn pos(chunk_x: i32, chunk_z: i32, x: f32, y: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(chunk_x, chunk_z),
            LocalPosition::new(Vec3::new(x, y, z)),
        )
    }

    fn spawn_wolf(world: &mut WorldData, catalog: &UnitCatalog, position: WorldPosition) -> UnitId {
        create_unit(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Authored,
        )
        .unwrap()
        .id
    }

    #[test]
    fn step_moves_toward_target() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 5.0);
        let unit_id = spawn_wolf(
            &mut world,
            &catalog,
            pos(0, 0, 0.0, 0.0, 0.0),
        );
        let target = pos(0, 0, 100.0, 0.0, 0.0);
        issue_unit_order(&mut world, unit_id, UnitOrder::MoveTo { target }).unwrap();

        let report = step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap();
        assert!(report.moved);
        assert!(!report.arrived);
        assert!(world.get_unit(unit_id).unwrap().placement.position.local.0.x > 0.0);
    }

    #[test]
    fn movement_speed_respects_move_speed_mps() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.0, 0.0, 0.0));
        let target = pos(0, 0, 100.0, 0.0, 0.0);
        issue_unit_order(&mut world, unit_id, UnitOrder::MoveTo { target }).unwrap();

        step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap();
        let speed = catalog.get(&UnitDefinitionId::new("wolf")).unwrap().move_speed_mps;
        assert!(
            (world.get_unit(unit_id).unwrap().placement.position.local.0.x - speed).abs() < 1e-3
        );
    }

    #[test]
    fn unit_arrives_and_becomes_idle() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 2.0);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.0, 0.0, 0.0));
        let target = pos(0, 0, 2.0, 0.0, 0.0);
        issue_unit_order(&mut world, unit_id, UnitOrder::MoveTo { target }).unwrap();

        let report = step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap();
        assert!(report.arrived);
        assert_eq!(world.get_unit(unit_id).unwrap().state, UnitState::Idle);
        assert_eq!(
            world.get_unit(unit_id).unwrap().placement.position.local.0.x,
            2.0
        );
    }

    #[test]
    fn y_updates_from_terrain_height() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 12.0);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.0, 0.0, 0.0));
        issue_unit_order(
            &mut world,
            unit_id,
            UnitOrder::MoveTo {
                target: pos(0, 0, 50.0, 0.0, 0.0),
            },
        )
        .unwrap();

        step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap();
        assert_eq!(
            world.get_unit(unit_id).unwrap().placement.position.local.0.y,
            12.0
        );
    }

    #[test]
    fn xz_update_rotation_source_metadata_preserved() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 1.0);
        let unit_id = world.allocate_unit_id();
        let rotation = Quat::from_rotation_y(1.1);
        let source = UnitSource::Procedural { seed: 5 };
        let mut record = UnitRecord::new(
            unit_id,
            UnitDefinitionId::new("wolf"),
            UnitPlacement::new(pos(0, 0, 0.0, 0.0, 0.0), rotation),
            source,
        );
        record.metadata = UnitMetadata;
        world
            .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();
        issue_unit_order(
            &mut world,
            unit_id,
            UnitOrder::MoveTo {
                target: pos(0, 0, 10.0, 0.0, 5.0),
            },
        )
        .unwrap();

        step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap();
        let updated = world.get_unit(unit_id).unwrap();
        assert!(updated.placement.position.local.0.x > 0.0);
        assert!(updated.placement.position.local.0.z > 0.0);
        assert_eq!(updated.placement.rotation, rotation);
        assert_eq!(updated.source, source);
        assert_eq!(updated.metadata, UnitMetadata);
    }

    #[test]
    fn cross_chunk_movement_updates_unit_index() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 1.0);
        insert_flat(&mut world, 1, 0, 1.0);
        let unit_id = spawn_wolf(
            &mut world,
            &catalog,
            pos(0, 0, 250.0, 0.0, 128.0),
        );
        issue_unit_order(
            &mut world,
            unit_id,
            UnitOrder::MoveTo {
                target: pos(1, 0, 10.0, 0.0, 128.0),
            },
        )
        .unwrap();

        step_unit_movement(&mut world, &catalog, unit_id, 10.0).unwrap();
        assert_eq!(
            world.unit_chunk(unit_id),
            Some(ChunkId::new(ChunkCoord::new(1, 0)))
        );
        world.assert_unit_index_consistent();
    }

    #[test]
    fn missing_terrain_prevents_movement_and_preserves_position() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 1.0, 0.0, 1.0));
        issue_unit_order(
            &mut world,
            unit_id,
            UnitOrder::MoveTo {
                target: pos(0, 0, 50.0, 0.0, 50.0),
            },
        )
        .unwrap();

        let before = world.get_unit(unit_id).unwrap().placement.position;
        let err = step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap_err();
        assert_eq!(err, UnitMovementError::TerrainUnavailable);
        assert_eq!(world.get_unit(unit_id).unwrap().placement.position, before);
    }

    #[test]
    fn missing_definition_reports_error() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 1.0);
        let unit_id = world.allocate_unit_id();
        world
            .insert_unit(
                ChunkId::new(ChunkCoord::new(0, 0)),
                UnitRecord::new(
                    unit_id,
                    UnitDefinitionId::new("missing"),
                    UnitPlacement::new(pos(0, 0, 0.0, 0.0, 0.0), Quat::IDENTITY),
                    UnitSource::Authored,
                ),
            )
            .unwrap();
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target: pos(0, 0, 10.0, 0.0, 0.0),
                },
            )
            .unwrap();

        let err = step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap_err();
        assert_eq!(err, UnitMovementError::DefinitionNotFound);
    }

    #[test]
    fn slope_too_steep_prevents_movement() {
        let mut samples = Vec::new();
        for _row in 0..3 {
            for col in 0..3 {
                samples.push(col as f32 * 40.0);
            }
        }
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        let catalog = UnitCatalog::from_definitions(vec![UnitDefinition::new(
            UnitDefinitionId::new("goat"),
            "Goat",
            "Wild",
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1.0,
            "Common",
            4.0,
            0.5,
            5.0,
            true,
            UnitRenderKey::reserved("goat"),
        )])
        .unwrap();
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("goat"),
            pos(0, 0, 100.0, 0.0, 128.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        issue_unit_order(
            &mut world,
            unit_id,
            UnitOrder::MoveTo {
                target: pos(0, 0, 150.0, 0.0, 128.0),
            },
        )
        .unwrap();

        let before = world.get_unit(unit_id).unwrap().placement.position;
        let err = step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap_err();
        assert_eq!(err, UnitMovementError::SlopeTooSteep);
        assert_eq!(world.get_unit(unit_id).unwrap().placement.position, before);
    }

    #[test]
    fn batch_movement_reports_counts() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let moving = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.0, 0.0, 0.0));
        let idle = spawn_wolf(&mut world, &catalog, pos(0, 0, 50.0, 0.0, 50.0));
        issue_unit_order(
            &mut world,
            moving,
            UnitOrder::MoveTo {
                target: pos(0, 0, 20.0, 0.0, 0.0),
            },
        )
        .unwrap();

        let report = step_all_unit_movement(&mut world, &catalog, 1.0);
        assert_eq!(report.moved, 1);
        assert_eq!(report.arrived, 0);
        assert_eq!(report.blocked_terrain_unavailable, 0);
        assert!(world.get_unit(idle).unwrap().placement.position.local.0.x == 50.0);
    }

    #[test]
    fn render_sync_reflects_moved_position() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<UnitCatalog>();
        app.init_resource::<crate::terrain::ChunkResidencyTracker>();
        app.init_resource::<UnitRenderIndex>();
        app.init_resource::<Assets<Scene>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.insert_resource(UnitSyncOverrides {
            treat_scenes_loaded: true,
        });
        app.add_systems(Update, sync_unit_render_entities);

        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        let scene = {
            let mut scenes = app.world_mut().resource_mut::<Assets<Scene>>();
            scenes.add(Scene::new(World::new()))
        };
        app.insert_resource(UnitSceneAssets::from_test_scenes(HashMap::from([(
            UnitDefinitionId::new("wolf"),
            scene,
        )])));

        let vertical_scale = 2.0;
        let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(StandardMaterial::default());
        app.world_mut().insert_resource(TerrainRenderAssets {
            material,
            vertical_scale,
        });

        let unit_id = {
            let catalog = UnitCatalog::default();
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_flat(&mut world, 0, 0, 6.0);
            let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.0, 0.0, 0.0));
            issue_unit_order(
                &mut world,
                unit_id,
                UnitOrder::MoveTo {
                    target: pos(0, 0, 20.0, 0.0, 0.0),
                },
            )
            .unwrap();
            step_unit_movement(&mut world, &catalog, unit_id, 1.0).unwrap();
            unit_id
        };

        app.world_mut()
            .resource_mut::<crate::terrain::ChunkResidencyTracker>()
            .mark_resident(chunk);
        app.update();

        let record = app.world().resource::<WorldData>().get_unit(unit_id).unwrap();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        let transform = app.world().entity(entity).get::<Transform>().unwrap();
        let expected = world_position_to_render_global(
            record.placement.position,
            app.world().resource::<WorldConfig>().chunk_layout(),
            vertical_scale,
        );
        assert_eq!(transform.translation, expected);
        assert!(record.placement.position.local.0.x > 0.0);
    }
}
