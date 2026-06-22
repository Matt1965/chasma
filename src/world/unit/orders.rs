//! Unit orders and issuance (ADR-030 U5).

use bevy::prelude::*;

use super::id::UnitId;
use super::state::UnitState;
use crate::world::{WorldData, WorldPosition};

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
}

/// Issue an order to a unit. Does not move immediately or pathfind.
pub fn issue_unit_order(
    world: &mut WorldData,
    unit_id: UnitId,
    order: UnitOrder,
) -> Result<(), UnitOrderError> {
    if world.get_unit(unit_id).is_none() {
        return Err(UnitOrderError::UnitNotFound);
    }

    let state = match order {
        UnitOrder::Idle => UnitState::Idle,
        UnitOrder::MoveTo { target } => UnitState::Moving { target },
    };

    world
        .set_unit_state(unit_id, state)
        .map_err(|_| UnitOrderError::UnitNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit, ChunkCoord, ChunkLayout, LocalPosition, UnitCatalog, UnitDefinitionId,
        UnitSource,
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

    #[test]
    fn issue_move_to_order_sets_moving_state() {
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

        let target = pos(100.0, 50.0);
        issue_unit_order(&mut world, unit_id, UnitOrder::MoveTo { target }).unwrap();

        assert_eq!(
            world.get_unit(unit_id).unwrap().state,
            UnitState::Moving { target }
        );
        assert_eq!(world.get_unit(unit_id).unwrap().placement.position, pos(10.0, 10.0));
    }

    #[test]
    fn issue_order_missing_unit() {
        let mut world = layout_world();
        let err = issue_unit_order(
            &mut world,
            UnitId::new(1),
            UnitOrder::MoveTo {
                target: pos(1.0, 1.0),
            },
        )
        .unwrap_err();
        assert_eq!(err, UnitOrderError::UnitNotFound);
    }
}
