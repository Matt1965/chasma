//! Authoritative unit movement along navigation paths (ADR-030 U5, ADR-032 U7, ADR-036 U11, ADR-037 U12).
//!
//! Steps [`UnitRecord`] placement on the XZ plane toward path waypoints,
//! grounding Y from resident heightfields. Local steering (U11) adjusts direction
//! without modifying paths. Command buffer resolution (U12) precedes each batch step.

use bevy::prelude::*;

use super::catalog::UnitCatalog;
use super::id::UnitId;
use super::orders::resolve_pending_unit_orders;
use super::state::UnitState;
use super::UnitInsertError;
use super::eligibility::unit_can_execute_actions;
use crate::world::combat::{
    step_all_combat_engagement, step_all_combat_strikes, step_combat_ai_acquisition,
    CombatAiReport, CombatAiScanState, CombatAiSettings, CombatStrikeReport,
};
use crate::world::projectile::{step_all_projectiles, ProjectileReport};
use super::death::step_unit_death_pipeline;
use crate::world::movement::feel::{
    should_skip_direction_smoothing, stabilized_movement_heading, steering_is_allowed,
    MovementFeelSettings, StabilizedMovementHeading,
};
use crate::world::movement::steering::SteeringSettings;
use crate::world::{
    apply_steering, ground_world_position, is_position_blocked_by_doodads,
    is_position_slope_walkable, xz_distance, AttackTargetingPolicy, ChunkLayout, DoodadCatalog,
    NavigationConfig, WeaponCatalog, WorldData, WorldPosition,
};
/// Distance below which a unit snaps to its move target (meters).
const ARRIVAL_DISTANCE_METERS: f32 = 0.05;
/// When blocked, treat as having reached a waypoint if within this distance (meters).
const WAYPOINT_SKIP_DISTANCE_METERS: f32 = 2.0;
/// When blocked near the final target, stop moving instead of freezing (meters).
const PARTIAL_ARRIVAL_DISTANCE_METERS: f32 = 2.5;

static STEERING_SETTINGS: SteeringSettings = SteeringSettings::DEFAULT;
static FEEL_SETTINGS: MovementFeelSettings = MovementFeelSettings::DEFAULT;

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
    pub blocked_by_doodad: u32,
    pub missing_definition: u32,
}

/// Movement tick plus command-buffer resolution and combat engagement metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UnitSimulationStepReport {
    pub movement: BatchUnitMovementReport,
    pub command_resolve: crate::world::CommandBufferResolveReport,
    pub combat: crate::world::CombatEngagementReport,
    pub combat_strike: crate::world::CombatStrikeReport,
    pub projectile: crate::world::ProjectileReport,
    pub death: crate::world::UnitDeathReport,
    pub combat_ai: CombatAiReport,
}

/// Why [`step_unit_movement`] could not complete a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitMovementError {
    UnitNotFound,
    DefinitionNotFound,
    TerrainUnavailable,
    SlopeUnavailable,
    SlopeTooSteep,
    BlockedByDoodad,
}

/// Advance one unit along its navigation path toward the current waypoint.
pub fn step_unit_movement(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    unit_id: UnitId,
    delta_seconds: f32,
) -> Result<UnitMovementStepReport, UnitMovementError> {
    let record = world
        .get_unit(unit_id)
        .ok_or(UnitMovementError::UnitNotFound)?;
    if !unit_can_execute_actions(world, unit_id) {
        return Ok(UnitMovementStepReport::default());
    }
    let definition_id = record.definition_id.clone();
    let UnitState::Moving {
        target,
        path,
        waypoint_index,
    } = record.state.clone()
    else {
        return Ok(UnitMovementStepReport::default());
    };
    let current_position = record.placement.position;

    let definition = unit_catalog
        .get(&definition_id)
        .ok_or(UnitMovementError::DefinitionNotFound)?;

    let layout = world.layout();
    let current_global = current_position.to_global(layout);
    let mut heading =
        stabilized_movement_heading(current_position, &path, waypoint_index, layout);
    let effective_index = heading
        .map(|h| h.waypoint_index)
        .unwrap_or(waypoint_index.min(path.len().saturating_sub(1)));
    let Some(waypoint) = path.waypoints.get(effective_index).copied() else {
        world
            .set_unit_state(unit_id, UnitState::Idle)
            .map_err(|_| UnitMovementError::UnitNotFound)?;
        world.movement_smoothing_mut().clear_unit(unit_id);
        return Ok(UnitMovementStepReport {
            moved: false,
            arrived: true,
        });
    };

    let waypoint_global = waypoint.to_global(layout);
    let mut to_waypoint = waypoint_global - current_global;
    to_waypoint.y = 0.0;
    let distance = to_waypoint.length();
    let step_distance = definition.move_speed_mps * delta_seconds;

    if heading.is_none() && distance > ARRIVAL_DISTANCE_METERS && distance > 1e-6 {
        heading = Some(StabilizedMovementHeading {
            waypoint_index: effective_index,
            direction_xz: Vec2::new(to_waypoint.x / distance, to_waypoint.z / distance),
        });
    }

    if heading.is_none() && distance <= ARRIVAL_DISTANCE_METERS {
        let next_index = effective_index + 1;
        if next_index >= path.len() {
            world
                .set_unit_state(unit_id, UnitState::Idle)
                .map_err(|_| UnitMovementError::UnitNotFound)?;
            world.movement_smoothing_mut().clear_unit(unit_id);
            return Ok(UnitMovementStepReport {
                moved: false,
                arrived: true,
            });
        }
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target,
                    path,
                    waypoint_index: next_index,
                },
            )
            .map_err(|_| UnitMovementError::UnitNotFound)?;
        return Ok(UnitMovementStepReport {
            moved: false,
            arrived: false,
        });
    }

    let Some(heading) = heading else {
        return Ok(UnitMovementStepReport {
            moved: false,
            arrived: false,
        });
    };

    let path_direction_xz = heading.direction_xz;
    let allow_steering = steering_is_allowed(Some(heading));
    let steered_direction_xz = apply_steering(
        world,
        unit_catalog,
        unit_id,
        current_position,
        path_direction_xz,
        definition.collision_radius_meters,
        target,
        delta_seconds,
        &STEERING_SETTINGS,
        allow_steering,
    );
    let skip_smoothing = should_skip_direction_smoothing(
        effective_index,
        path.len(),
        distance,
        step_distance,
    );
    let movement_direction_xz = if skip_smoothing {
        if steered_direction_xz.length_squared() > 1e-8 {
            steered_direction_xz.normalize()
        } else {
            steered_direction_xz
        }
    } else {
        world
            .movement_smoothing_mut()
            .smooth_direction(unit_id, steered_direction_xz, &FEEL_SETTINGS)
    };

    let destination_global = if distance <= step_distance.max(ARRIVAL_DISTANCE_METERS) {
        Vec3::new(waypoint_global.x, current_global.y, waypoint_global.z)
    } else {
        let direction = Vec3::new(movement_direction_xz.x, 0.0, movement_direction_xz.y);
        current_global + direction * step_distance
    };

    let candidate = WorldPosition::from_global(destination_global, layout);
    let grounded = ground_world_position(world, candidate).ok_or(UnitMovementError::TerrainUnavailable)?;

    if !is_position_slope_walkable(world, grounded, definition.max_slope_degrees) {
        return handle_blocked_step(
            world,
            unit_id,
            target,
            path,
            waypoint_index,
            current_position,
            layout,
        );
    }

    if is_position_blocked_by_doodads(
        world,
        doodad_catalog,
        grounded,
        definition.collision_radius_meters,
    ) {
        return handle_blocked_step(
            world,
            unit_id,
            target,
            path,
            waypoint_index,
            current_position,
            layout,
        );
    }

    world
        .relocate_unit(unit_id, grounded)
        .map_err(|error| match error {
            UnitInsertError::UnitNotFound => UnitMovementError::UnitNotFound,
            UnitInsertError::ChunkPlacementMismatch => UnitMovementError::TerrainUnavailable,
        })?;

    let grounded_global = grounded.to_global(layout);
    let mut to_waypoint_after = waypoint_global - grounded_global;
    to_waypoint_after.y = 0.0;
    let distance_after = to_waypoint_after.length();
    let reached_waypoint = distance_after <= ARRIVAL_DISTANCE_METERS
        || distance <= step_distance.max(ARRIVAL_DISTANCE_METERS);
    if reached_waypoint {
        let next_index = effective_index + 1;
        if next_index >= path.len() {
            world
                .set_unit_state(unit_id, UnitState::Idle)
                .map_err(|_| UnitMovementError::UnitNotFound)?;
            world.movement_smoothing_mut().clear_unit(unit_id);
            Ok(UnitMovementStepReport {
                moved: true,
                arrived: true,
            })
        } else {
            world
                .set_unit_state(
                    unit_id,
                    UnitState::Moving {
                        target,
                        path,
                        waypoint_index: next_index,
                    },
                )
                .map_err(|_| UnitMovementError::UnitNotFound)?;
            Ok(UnitMovementStepReport {
                moved: true,
                arrived: false,
            })
        }
    } else {
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target,
                    path,
                    waypoint_index: effective_index,
                },
            )
            .map_err(|_| UnitMovementError::UnitNotFound)?;
        Ok(UnitMovementStepReport {
            moved: true,
            arrived: false,
        })
    }
}

/// Advance all units deterministically by [`UnitId`] (REVIEW-A4 canonical tick order).
///
/// 1. Resolve pending unit orders
/// 2. Combat engagement (range / chase / target validation)
/// 3. Combat strikes (may spawn projectiles)
/// 4. Projectile movement / impact (skips same-tick spawns)
/// 5. Death detection, queue, target cleanup, removal
/// 6. Combat AI acquisition (post-cleanup snapshot)
/// 7. Unit movement
pub fn step_all_unit_movement(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    targeting_policy: AttackTargetingPolicy,
    combat_ai_settings: &CombatAiSettings,
    combat_ai_scan: &mut CombatAiScanState,
    delta_seconds: f32,
    simulation_tick: u64,
) -> UnitSimulationStepReport {
    let command_resolve =
        resolve_pending_unit_orders(world, unit_catalog, doodad_catalog, nav_config);
    let mut combat_strike = CombatStrikeReport::default();
    let combat = step_all_combat_engagement(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        targeting_policy,
        &mut combat_strike,
    );
    let mut projectile_spawn = ProjectileReport::default();
    combat_strike = step_all_combat_strikes(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        targeting_policy,
        delta_seconds,
        &mut projectile_spawn,
    );
    let spawned_this_tick = projectile_spawn.spawned_projectile_ids();
    let mut projectile_step =
        step_all_projectiles(world, delta_seconds, &spawned_this_tick);
    let mut projectile = projectile_spawn;
    projectile.traces.append(&mut projectile_step.traces);
    let death = step_unit_death_pipeline(world, simulation_tick);
    let combat_ai = step_combat_ai_acquisition(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        targeting_policy,
        combat_ai_settings,
        combat_ai_scan,
        delta_seconds,
    );
    let mut report = BatchUnitMovementReport::default();
    for unit_id in world.sorted_unit_ids() {
        match step_unit_movement(world, unit_catalog, doodad_catalog, unit_id, delta_seconds) {
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
            Err(UnitMovementError::BlockedByDoodad) => report.blocked_by_doodad += 1,
            Err(UnitMovementError::UnitNotFound) => {}
        }
    }
    UnitSimulationStepReport {
        movement: report,
        command_resolve,
        combat,
        combat_strike,
        projectile,
        death,
        combat_ai,
    }
}

fn handle_blocked_step(
    world: &mut WorldData,
    unit_id: UnitId,
    target: WorldPosition,
    path: crate::world::NavigationPath,
    waypoint_index: usize,
    current_position: WorldPosition,
    layout: ChunkLayout,
) -> Result<UnitMovementStepReport, UnitMovementError> {
    let Some(waypoint) = path.waypoints.get(waypoint_index).copied() else {
        world
            .set_unit_state(unit_id, UnitState::Idle)
            .map_err(|_| UnitMovementError::UnitNotFound)?;
        return Ok(UnitMovementStepReport {
            moved: false,
            arrived: true,
        });
    };

    let dist_to_waypoint = xz_distance(current_position, waypoint, layout);
    let dist_to_target = xz_distance(current_position, target, layout);

    if dist_to_waypoint <= WAYPOINT_SKIP_DISTANCE_METERS && waypoint_index + 1 < path.len() {
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target,
                    path,
                    waypoint_index: waypoint_index + 1,
                },
            )
            .map_err(|_| UnitMovementError::UnitNotFound)?;
        return Ok(UnitMovementStepReport {
            moved: false,
            arrived: false,
        });
    }

    let arrived = dist_to_target <= PARTIAL_ARRIVAL_DISTANCE_METERS;
    world
        .set_unit_state(unit_id, UnitState::Idle)
        .map_err(|_| UnitMovementError::UnitNotFound)?;
    Ok(UnitMovementStepReport {
        moved: false,
        arrived,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{world_position_to_render_global, TerrainRenderAssets};
    use crate::units::{
        sync_unit_render_entities, UnitRenderIndex, UnitSceneAssets, UnitSyncOverrides,
    };
    use crate::world::movement::feel::stabilized_movement_heading;
    use crate::world::{
        create_doodad, create_unit, issue_unit_order, resolve_all_pending_unit_orders,
        resolve_pending_unit_orders, ChunkCoord,
        ChunkData, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId, DoodadPlacementOverrides,
        DoodadSource, Heightfield, LocalPosition, NavigationConfig, NavigationPath, UnitCatalog,
        UnitDefinition, UnitDefinitionId, UnitMetadata, UnitOrder, UnitPlacement, UnitRecord,
        UnitRenderKey, UnitSource, WeaponCatalog, WorldConfig,
    };
    use crate::world::AttackTargetingPolicy;
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

    /// Finer heightfield so slope sampling works across the full chunk width (ADR-029).
    fn flat_chunk_dense(height: f32) -> ChunkData {
        let edge: u32 = 65;
        let count = edge as usize * edge as usize;
        let heightfield = Heightfield::from_samples(edge, 4.0, vec![height; count]).unwrap();
        ChunkData::new(heightfield, Vec::new())
    }

    fn insert_flat(world: &mut WorldData, x: i32, z: i32, height: f32) -> ChunkId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        world.insert(chunk, flat_chunk(height));
        chunk
    }

    fn insert_flat_dense(world: &mut WorldData, x: i32, z: i32, height: f32) -> ChunkId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        world.insert(chunk, flat_chunk_dense(height));
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

    fn nav_config() -> NavigationConfig {
        NavigationConfig::default()
    }

    fn step_all(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        doodad_catalog: &DoodadCatalog,
        delta_seconds: f32,
    ) -> BatchUnitMovementReport {
        let mut scan = CombatAiScanState::default();
        let settings = CombatAiSettings::default();
        step_all_unit_movement(
            world,
            catalog,
            &WeaponCatalog::default(),
            doodad_catalog,
            &nav_config(),
            AttackTargetingPolicy::default(),
            &settings,
            &mut scan,
            delta_seconds,
            0,
        )
        .movement
    }

    fn weapons() -> crate::world::WeaponCatalog {
        crate::world::WeaponCatalog::default()
    }

    fn issue_move(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        doodad_catalog: &DoodadCatalog,
        unit_id: UnitId,
        target: WorldPosition,
    ) {
        issue_unit_order(
            world,
            catalog,
            &weapons(),
            doodad_catalog,
            &nav_config(),
            unit_id,
            UnitOrder::MoveTo { target },
            crate::world::AttackTargetingPolicy::default(),
        )
        .unwrap();
        resolve_all_pending_unit_orders(world, catalog, doodad_catalog, &nav_config());
    }

    fn moving_state(target: WorldPosition) -> UnitState {
        UnitState::Moving {
            target,
            path: NavigationPath::new(vec![target]),
            waypoint_index: 0,
        }
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
        issue_move(&mut world, &catalog, &DoodadCatalog::default(), unit_id, target);

        let report = step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap();
        assert!(report.moved);
        assert!(!report.arrived);
        assert!(world.get_unit(unit_id).unwrap().placement.position.local.0.x > 0.0);
    }

    #[test]
    fn movement_speed_respects_move_speed_mps() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let start = pos(0, 0, 20.0, 0.0, 20.0);
        let unit_id = spawn_wolf(&mut world, &catalog, start);
        let speed = catalog.get(&UnitDefinitionId::new("wolf")).unwrap().move_speed_mps;
        world
            .set_unit_state(
                unit_id,
                moving_state(pos(0, 0, 20.0 + speed, 0.0, 20.0)),
            )
            .unwrap();

        step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap();
        let after = world.get_unit(unit_id).unwrap().placement.position.local.0;
        assert!((after.x - (20.0 + speed)).abs() < 1e-3);
        assert!((after.z - 20.0).abs() < 1e-3);
    }

    #[test]
    fn unit_arrives_and_becomes_idle() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 2.0);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.0, 0.0, 0.0));
        let target = pos(0, 0, 2.0, 0.0, 0.0);
        issue_move(&mut world, &catalog, &DoodadCatalog::default(), unit_id, target);

        let report = step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap();
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
        issue_move(
            &mut world,
            &catalog,
            &DoodadCatalog::default(),
            unit_id,
            pos(0, 0, 50.0, 0.0, 0.0),
        );

        step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap();
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
            crate::world::default_ownership_for_source(source),
            5,
        );
        world
            .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();
        issue_move(
            &mut world,
            &catalog,
            &DoodadCatalog::default(),
            unit_id,
            pos(0, 0, 10.0, 0.0, 5.0),
        );

        step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap();
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
        insert_flat_dense(&mut world, 0, 0, 1.0);
        insert_flat_dense(&mut world, 1, 0, 1.0);
        let unit_id = spawn_wolf(
            &mut world,
            &catalog,
            pos(0, 0, 200.0, 0.0, 128.0),
        );
        world
            .set_unit_state(
                unit_id,
                moving_state(pos(1, 0, 50.0, 0.0, 128.0)),
            )
            .unwrap();

        step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 20.0)
            .unwrap();
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
        insert_flat(&mut world, 0, 0, 1.0);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 1.0, 0.0, 1.0));
        let target = pos(0, 0, 50.0, 0.0, 50.0);
        world.remove(ChunkId::new(ChunkCoord::new(0, 0)));
        world
            .set_unit_state(unit_id, moving_state(target))
            .unwrap();

        let before = world.get_unit(unit_id).unwrap().placement.position;
        let err = step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap_err();
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
                    crate::world::UnitOwnership::neutral(),
                    10,
                ),
            )
            .unwrap();
        world
            .set_unit_state(unit_id, moving_state(pos(0, 0, 10.0, 0.0, 0.0)))
            .unwrap();

        let err = step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap_err();
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
            1,
            1.0,
            "Common",
            4.0,
            0.5,
            5.0,
            crate::world::WeaponDefinitionId::new("weapon_fists"),
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
        world
            .set_unit_state(
                unit_id,
                moving_state(pos(0, 0, 150.0, 0.0, 128.0)),
            )
            .unwrap();

        let before = world.get_unit(unit_id).unwrap().placement.position;
        let report = step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap();
        assert!(!report.moved);
        assert_eq!(world.get_unit(unit_id).unwrap().placement.position, before);
        assert_eq!(world.get_unit(unit_id).unwrap().state, UnitState::Idle);
    }

    #[test]
    fn batch_movement_reports_counts() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let moving = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.0, 0.0, 0.0));
        let idle = spawn_wolf(&mut world, &catalog, pos(0, 0, 50.0, 0.0, 50.0));
        issue_move(
            &mut world,
            &catalog,
            &DoodadCatalog::default(),
            moving,
            pos(0, 0, 20.0, 0.0, 0.0),
        );

        let report = step_all(&mut world, &catalog, &DoodadCatalog::default(), 1.0);
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
            issue_move(
                &mut world,
                &catalog,
                &DoodadCatalog::default(),
                unit_id,
                pos(0, 0, 20.0, 0.0, 0.0),
            );
            step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap();
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

    #[test]
    fn movement_routes_around_blocked_doodad() {
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 1.0);
        for z in 0..16 {
            create_doodad(
                &doodad_catalog,
                &mut world,
                &DoodadDefinitionId::new("tree_oak"),
                pos(0, 0, 20.0, 0.0, z as f32 * 4.0),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();
        }
        let unit_id = spawn_wolf(&mut world, &unit_catalog, pos(0, 0, 4.0, 0.0, 28.0));
        issue_move(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            unit_id,
            pos(0, 0, 36.0, 0.0, 28.0),
        );

        let before = world.get_unit(unit_id).unwrap().placement.position;
        let report = step_unit_movement(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            unit_id,
            1.0,
        )
        .unwrap();
        assert!(report.moved);
        assert_ne!(world.get_unit(unit_id).unwrap().placement.position, before);
    }

    #[test]
    fn unit_arrives_at_exact_clicked_target() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 4.0, 0.0, 4.0));
        let target = pos(0, 0, 37.0, 0.0, 19.0);
        issue_move(
            &mut world,
            &catalog,
            &DoodadCatalog::default(),
            unit_id,
            target,
        );

        for _ in 0..512 {
            let report = step_unit_movement(
                &mut world,
                &catalog,
                &DoodadCatalog::default(),
                unit_id,
                0.25,
            )
            .unwrap();
            if report.arrived {
                break;
            }
        }

        assert!(matches!(
            world.get_unit(unit_id).unwrap().state,
            UnitState::Idle
        ));
        let final_pos = world.get_unit(unit_id).unwrap().placement.position;
        let final_global = final_pos.to_global(layout());
        assert!((final_global.x - 37.0).abs() < 0.15);
        assert!((final_global.z - 19.0).abs() < 0.15);
    }

    #[test]
    fn batch_movement_moves_around_blocked_doodad() {
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        for z in 0..16 {
            create_doodad(
                &doodad_catalog,
                &mut world,
                &DoodadDefinitionId::new("tree_oak"),
                pos(0, 0, 20.0, 0.0, z as f32 * 4.0),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();
        }
        let unit_id = spawn_wolf(&mut world, &unit_catalog, pos(0, 0, 4.0, 0.0, 28.0));
        issue_move(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            unit_id,
            pos(0, 0, 36.0, 0.0, 28.0),
        );

        let report = step_all(&mut world, &unit_catalog, &doodad_catalog, 1.0);
        assert_eq!(report.blocked_by_doodad, 0);
        assert_eq!(report.moved, 1);
    }

    fn xz_distance_between_units(world: &WorldData, a: UnitId, b: UnitId) -> f32 {
        let layout = world.layout();
        let ga = world.get_unit(a).unwrap().placement.position.to_global(layout);
        let gb = world.get_unit(b).unwrap().placement.position.to_global(layout);
        Vec2::new(ga.x - gb.x, ga.z - gb.z).length()
    }

    fn spawn_wolves_at(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        positions: &[(f32, f32)],
    ) -> Vec<UnitId> {
        positions
            .iter()
            .map(|&(x, z)| spawn_wolf(world, catalog, pos(0, 0, x, 0.0, z)))
            .collect()
    }

    #[test]
    fn clustered_units_separate_during_movement() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let ids = spawn_wolves_at(
            &mut world,
            &catalog,
            &[(10.0, 10.0), (10.2, 10.0), (10.1, 10.15)],
        );
        let shared_target = pos(0, 0, 40.0, 0.0, 40.0);
        for &unit_id in &ids {
            issue_move(
                &mut world,
                &catalog,
                &DoodadCatalog::default(),
                unit_id,
                shared_target,
            );
        }

        for _ in 0..32 {
            step_all(&mut world, &catalog, &DoodadCatalog::default(), 0.25);
        }

        let d01 = xz_distance_between_units(&world, ids[0], ids[1]);
        let d02 = xz_distance_between_units(&world, ids[0], ids[2]);
        let min_radius = catalog
            .get(&UnitDefinitionId::new("wolf"))
            .unwrap()
            .collision_radius_meters
            * 2.0;
        assert!(d01 >= min_radius * 0.85);
        assert!(d02 >= min_radius * 0.85);
    }

    #[test]
    fn steering_does_not_modify_move_target() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let a = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.0, 0.0, 0.0));
        let b = spawn_wolf(&mut world, &catalog, pos(0, 0, 0.5, 0.0, 0.0));
        let target = pos(0, 0, 30.0, 0.0, 0.0);
        issue_move(&mut world, &catalog, &DoodadCatalog::default(), a, target);
        issue_move(&mut world, &catalog, &DoodadCatalog::default(), b, target);

        let before_a = match world.get_unit(a).unwrap().state {
            UnitState::Moving { target, .. } => target,
            _ => panic!("expected moving"),
        };

        step_all(&mut world, &catalog, &DoodadCatalog::default(), 0.25);

        let after_a = match world.get_unit(a).unwrap().state {
            UnitState::Moving { target, .. } => target,
            _ => panic!("expected moving"),
        };
        assert_eq!(before_a, after_a);
    }

    #[test]
    fn single_unit_path_unchanged_without_neighbors() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 20.0, 0.0, 20.0));
        world
            .set_unit_state(
                unit_id,
                moving_state(pos(0, 0, 20.0 + 4.0, 0.0, 20.0)),
            )
            .unwrap();

        step_unit_movement(&mut world, &catalog, &DoodadCatalog::default(), unit_id, 1.0).unwrap();
        let after = world.get_unit(unit_id).unwrap().placement.position.local.0;
        assert!((after.x - 24.0).abs() < 1e-3);
        assert!((after.z - 20.0).abs() < 1e-3);
    }

    #[test]
    fn two_units_maintain_separation_while_moving() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let a = spawn_wolf(&mut world, &catalog, pos(0, 0, 20.0, 0.0, 20.0));
        let b = spawn_wolf(&mut world, &catalog, pos(0, 0, 20.05, 0.0, 20.0));
        issue_move(&mut world, &catalog, &doodad_catalog, a, pos(0, 0, 200.0, 0.0, 20.0));
        issue_move(&mut world, &catalog, &doodad_catalog, b, pos(0, 0, 200.0, 0.0, 22.0));

        for _ in 0..48 {
            step_all(&mut world, &catalog, &doodad_catalog, 0.25);
        }

        let min_radius = catalog
            .get(&UnitDefinitionId::new("wolf"))
            .unwrap()
            .collision_radius_meters
            * 2.0;
        let final_distance = xz_distance_between_units(&world, a, b);
        assert!(final_distance >= min_radius * 0.85);
    }

    #[test]
    fn formation_arrival_does_not_collapse_to_single_point() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav = NavigationConfig::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let ids = spawn_wolves_at(
            &mut world,
            &catalog,
            &[(5.0, 5.0), (8.0, 8.0), (11.0, 11.0)],
        );
        let click = pos(0, 0, 50.0, 0.0, 50.0);
        let plan = crate::world::FormationPlanner::plan_move(
            crate::world::FormationKind::Circle,
            &ids,
            click,
            &world,
            &catalog,
            layout(),
        );
        for assignment in plan.assignments {
            issue_unit_order(
                &mut world,
                &catalog,
                &crate::world::WeaponCatalog::default(),
                &doodad_catalog,
                &nav,
                assignment.unit_id,
                UnitOrder::MoveTo {
                    target: assignment.target,
                },
                crate::world::AttackTargetingPolicy::default(),
            )
            .unwrap();
        }
        resolve_all_pending_unit_orders(&mut world, &catalog, &doodad_catalog, &nav);

        for _ in 0..512 {
            step_all(&mut world, &catalog, &doodad_catalog, 0.25);
            if ids.iter().all(|id| matches!(world.get_unit(*id).unwrap().state, UnitState::Idle)) {
                break;
            }
        }

        let layout = world.layout();
        let positions: Vec<_> = ids
            .iter()
            .map(|id| {
                let g = world.get_unit(*id).unwrap().placement.position.to_global(layout);
                Vec2::new(g.x, g.z)
            })
            .collect();
        assert!((positions[0] - positions[1]).length() > 0.5);
        assert!((positions[1] - positions[2]).length() > 0.5);
    }

    #[test]
    fn steering_avoids_position_oscillation() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let a = spawn_wolf(&mut world, &catalog, pos(0, 0, 10.0, 0.0, 10.0));
        let b = spawn_wolf(&mut world, &catalog, pos(0, 0, 10.4, 0.0, 10.0));
        let target = pos(0, 0, 50.0, 0.0, 10.0);
        issue_move(&mut world, &catalog, &DoodadCatalog::default(), a, target);
        issue_move(&mut world, &catalog, &DoodadCatalog::default(), b, target);

        let mut previous = xz_distance_between_units(&world, a, b);
        for _ in 0..20 {
            step_all(&mut world, &catalog, &DoodadCatalog::default(), 0.1);
            let current = xz_distance_between_units(&world, a, b);
            assert!(current >= previous * 0.75);
            previous = current;
        }
    }

    #[test]
    fn first_movement_direction_matches_first_waypoint() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let start = pos(0, 0, 20.0, 0.0, 20.0);
        let unit_id = spawn_wolf(&mut world, &catalog, start);
        let goal = pos(0, 0, 80.0, 0.0, 35.0);
        issue_move(&mut world, &catalog, &doodad_catalog, unit_id, goal);

        let UnitState::Moving { ref path, .. } = world.get_unit(unit_id).unwrap().state else {
            panic!("expected moving");
        };
        let layout = world.layout();
        let heading = stabilized_movement_heading(start, path, 0, layout).expect("heading");
        let before = world.get_unit(unit_id).unwrap().placement.position.to_global(layout);

        step_unit_movement(
            &mut world,
            &catalog,
            &doodad_catalog,
            unit_id,
            0.25,
        )
        .unwrap();

        let after = world
            .get_unit(unit_id)
            .unwrap()
            .placement
            .position
            .to_global(layout);
        let delta = Vec2::new(after.x - before.x, after.z - before.z);
        assert!(delta.length() > 0.01);
        let moved_dir = delta.normalize();
        assert!(
            moved_dir.dot(heading.direction_xz) > 0.95,
            "first step dir {moved_dir:?} expected ~ {:?}",
            heading.direction_xz
        );
    }

    #[test]
    fn queued_order_does_not_move_before_buffer_resolution() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0, 0.0);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0, 0, 10.0, 0.0, 10.0));
        let before = world.get_unit(unit_id).unwrap().placement.position;

        issue_unit_order(
            &mut world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &doodad_catalog,
            &nav_config(),
            unit_id,
            UnitOrder::MoveTo {
                target: pos(0, 0, 60.0, 0.0, 10.0),
            },
            crate::world::AttackTargetingPolicy::default(),
        )
        .unwrap();

        step_unit_movement(&mut world, &catalog, &doodad_catalog, unit_id, 0.25).unwrap();
        assert_eq!(world.get_unit(unit_id).unwrap().placement.position, before);
        assert_eq!(world.get_unit(unit_id).unwrap().state, UnitState::Idle);
    }

    #[test]
    fn first_step_velocity_is_deterministic() {
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let mut world_a = WorldData::new(layout());
        let mut world_b = WorldData::new(layout());
        insert_flat(&mut world_a, 0, 0, 0.0);
        insert_flat(&mut world_b, 0, 0, 0.0);
        let id_a = spawn_wolf(&mut world_a, &catalog, pos(0, 0, 12.0, 0.0, 12.0));
        let id_b = spawn_wolf(&mut world_b, &catalog, pos(0, 0, 12.0, 0.0, 12.0));
        let target = pos(0, 0, 70.0, 0.0, 12.0);
        issue_move(&mut world_a, &catalog, &doodad_catalog, id_a, target);
        issue_move(&mut world_b, &catalog, &doodad_catalog, id_b, target);

        step_unit_movement(&mut world_a, &catalog, &doodad_catalog, id_a, 0.25).unwrap();
        step_unit_movement(&mut world_b, &catalog, &doodad_catalog, id_b, 0.25).unwrap();

        assert_eq!(
            world_a.get_unit(id_a).unwrap().placement.position,
            world_b.get_unit(id_b).unwrap().placement.position
        );
    }
}
