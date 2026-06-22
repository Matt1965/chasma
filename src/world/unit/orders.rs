//! Unit orders and issuance (ADR-030 U5, ADR-032 U7).

use bevy::prelude::*;

use super::catalog::UnitCatalog;
use super::id::UnitId;
use super::state::UnitState;
use crate::world::{
    find_path, DoodadCatalog, NavigationConfig, NavigationError, WorldData, WorldPosition,
};

/// Authoritative command issued to a unit instance.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum UnitOrder {
    Idle,
    MoveTo {
        target: WorldPosition,
    },
}

/// Why [`issue_unit_order`] failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitOrderError {
    UnitNotFound,
    DefinitionNotFound,
    PathStartBlocked,
    PathGoalBlocked,
    NoPath,
    PathTerrainUnavailable,
}

/// Issue an order to a unit. `MoveTo` computes a navigation path before moving.
pub fn issue_unit_order(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    unit_id: UnitId,
    order: UnitOrder,
) -> Result<(), UnitOrderError> {
    let record = world.get_unit(unit_id).ok_or(UnitOrderError::UnitNotFound)?;
    let definition_id = record.definition_id.clone();
    let start = record.placement.position;

    let state = match order {
        UnitOrder::Idle => UnitState::Idle,
        UnitOrder::MoveTo { target } => {
            let definition = unit_catalog
                .get(&definition_id)
                .ok_or(UnitOrderError::DefinitionNotFound)?;
            let path = find_path(
                world,
                doodad_catalog,
                nav_config,
                definition.collision_radius_meters,
                start,
                target,
            )
            .map_err(map_navigation_error)?;
            UnitState::Moving {
                target,
                path,
                waypoint_index: 0,
            }
        }
    };

    world
        .set_unit_state(unit_id, state)
        .map_err(|_| UnitOrderError::UnitNotFound)
}

fn map_navigation_error(error: NavigationError) -> UnitOrderError {
    match error {
        NavigationError::StartBlocked => UnitOrderError::PathStartBlocked,
        NavigationError::GoalBlocked => UnitOrderError::PathGoalBlocked,
        NavigationError::NoPath => UnitOrderError::NoPath,
        NavigationError::TerrainUnavailable => UnitOrderError::PathTerrainUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        UnitDefinitionId, UnitSource,
    };

    fn layout_world() -> WorldData {
        WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn insert_flat(world: &mut WorldData) {
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn issue(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        unit_id: UnitId,
        order: UnitOrder,
    ) -> Result<(), UnitOrderError> {
        issue_unit_order(
            world,
            catalog,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            unit_id,
            order,
        )
    }

    #[test]
    fn issue_move_to_order_sets_moving_state_with_path() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        insert_flat(&mut world);
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let target = pos(100.0, 50.0);
        issue(&mut world, &catalog, unit_id, UnitOrder::MoveTo { target }).unwrap();

        let record = world.get_unit(unit_id).unwrap();
        match record.state {
            UnitState::Moving {
                target: stored_target,
                ref path,
                waypoint_index,
            } => {
                assert_eq!(stored_target, target);
                assert!(!path.is_empty());
                assert_eq!(waypoint_index, 0);
            }
            _ => panic!("expected Moving state"),
        }
        assert_eq!(record.placement.position, pos(10.0, 10.0));
    }

    #[test]
    fn issue_order_missing_unit() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        insert_flat(&mut world);
        let err = issue(
            &mut world,
            &catalog,
            UnitId::new(1),
            UnitOrder::MoveTo {
                target: pos(1.0, 1.0),
            },
        )
        .unwrap_err();
        assert_eq!(err, UnitOrderError::UnitNotFound);
    }

    #[test]
    fn issue_move_without_terrain_fails() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let err = issue(
            &mut world,
            &catalog,
            unit_id,
            UnitOrder::MoveTo {
                target: pos(100.0, 50.0),
            },
        )
        .unwrap_err();
        assert_eq!(err, UnitOrderError::PathTerrainUnavailable);
    }
}
