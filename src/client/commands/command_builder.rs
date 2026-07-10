//! Command builder — [`ContextualCommandIntent`] → executable plan (ADR-041, REVIEW-B3).

use crate::units::input::SelectedUnits;
use crate::world::{UnitId, WorldData, WorldPosition};

use super::command_availability::CommandUnavailableReason;
use super::command_types::{CommandTarget, CommandType, ContextualCommandIntent};

/// Executable command plan produced by the builder (before [`issue_unit_order`]).
#[derive(Debug, Clone, PartialEq)]
pub enum BuiltCommandPlan {
    MoveTo { target: WorldPosition },
    Attack { target: UnitId },
    AttackMove { destination: WorldPosition },
    StopAll,
    NoOp,
}

/// Why command building failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandBuildError {
    EmptySelection,
    TargetUnitNotFound,
    MissingMoveTarget,
    MissingAttackTarget,
    FeatureUnavailable(CommandUnavailableReason),
}

/// Translate a contextual intent into a simulation-facing plan.
pub fn build_command_plan(
    intent: &ContextualCommandIntent,
    selection: &SelectedUnits,
    world: &WorldData,
) -> Result<BuiltCommandPlan, CommandBuildError> {
    if selection.is_empty() {
        return Err(CommandBuildError::EmptySelection);
    }

    match intent.command_type {
        CommandType::Move => {
            let target = resolve_move_target(&intent.target, world)?;
            Ok(BuiltCommandPlan::MoveTo { target })
        }
        CommandType::Attack => {
            let target = resolve_attack_target(&intent.target)?;
            Ok(BuiltCommandPlan::Attack { target })
        }
        CommandType::AttackMove => {
            let destination = resolve_move_target(&intent.target, world)?;
            Ok(BuiltCommandPlan::AttackMove { destination })
        }
        CommandType::Stop => Ok(BuiltCommandPlan::StopAll),
        CommandType::HoldPosition | CommandType::Interact => Err(CommandBuildError::FeatureUnavailable(
            CommandUnavailableReason::FeatureNotImplemented,
        )),
    }
}

/// Safe fallback for unknown or future command types (unit tests only).
#[cfg(test)]
pub fn build_command_plan_or_fallback_move(
    intent: &ContextualCommandIntent,
    selection: &SelectedUnits,
    world: &WorldData,
    fallback_target: Option<WorldPosition>,
) -> BuiltCommandPlan {
    match build_command_plan(intent, selection, world) {
        Ok(plan) => plan,
        Err(_) => fallback_target
            .map(|target| BuiltCommandPlan::MoveTo { target })
            .unwrap_or(BuiltCommandPlan::NoOp),
    }
}

fn resolve_move_target(
    target: &CommandTarget,
    world: &WorldData,
) -> Result<WorldPosition, CommandBuildError> {
    match target {
        CommandTarget::Terrain { position } => Ok(*position),
        CommandTarget::Unit { unit_id } => world
            .get_unit(*unit_id)
            .map(|record| record.placement.position)
            .ok_or(CommandBuildError::TargetUnitNotFound),
    }
}

fn resolve_attack_target(target: &CommandTarget) -> Result<UnitId, CommandBuildError> {
    match target {
        CommandTarget::Unit { unit_id } => Ok(*unit_id),
        CommandTarget::Terrain { .. } => Err(CommandBuildError::MissingAttackTarget),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        UnitDefinitionId, UnitSource, WorldPosition,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
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
    fn builder_produces_move_to_for_terrain() {
        let world = flat_world();
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let intent = ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Terrain { position: pos(20.0, 30.0) },
        };
        let plan = build_command_plan(&intent, &selection, &world).unwrap();
        assert_eq!(plan, BuiltCommandPlan::MoveTo { target: pos(20.0, 30.0) });
    }

    #[test]
    fn attack_move_stores_destination() {
        let world = flat_world();
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let intent = ContextualCommandIntent {
            command_type: CommandType::AttackMove,
            target: CommandTarget::Terrain { position: pos(5.0, 5.0) },
        };
        let plan = build_command_plan(&intent, &selection, &world).unwrap();
        assert_eq!(
            plan,
            BuiltCommandPlan::AttackMove {
                destination: pos(5.0, 5.0)
            }
        );
    }

    #[test]
    fn attack_requires_unit_target() {
        let world = flat_world();
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let intent = ContextualCommandIntent {
            command_type: CommandType::Attack,
            target: CommandTarget::Terrain { position: pos(5.0, 5.0) },
        };
        assert_eq!(
            build_command_plan(&intent, &selection, &world),
            Err(CommandBuildError::MissingAttackTarget)
        );
    }

    #[test]
    fn hold_position_rejects_as_unimplemented() {
        let world = flat_world();
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let intent = ContextualCommandIntent {
            command_type: CommandType::HoldPosition,
            target: CommandTarget::Terrain { position: pos(0.0, 0.0) },
        };
        assert!(matches!(
            build_command_plan(&intent, &selection, &world),
            Err(CommandBuildError::FeatureUnavailable(
                CommandUnavailableReason::FeatureNotImplemented
            ))
        ));
    }

    #[test]
    fn interact_rejects_as_unimplemented() {
        let world = flat_world();
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let intent = ContextualCommandIntent {
            command_type: CommandType::Interact,
            target: CommandTarget::Terrain { position: pos(0.0, 0.0) },
        };
        assert!(matches!(
            build_command_plan(&intent, &selection, &world),
            Err(CommandBuildError::FeatureUnavailable(
                CommandUnavailableReason::FeatureNotImplemented
            ))
        ));
    }

    #[test]
    fn unit_target_move_uses_unit_position() {
        let catalog = crate::world::UnitCatalog::default();
        let mut world = flat_world();
        let target_unit = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(12.0, 14.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let intent = ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Unit {
                unit_id: target_unit,
            },
        };
        let plan = build_command_plan(&intent, &selection, &world).unwrap();
        assert_eq!(plan, BuiltCommandPlan::MoveTo { target: pos(12.0, 14.0) });
    }

    #[test]
    fn stop_produces_stop_all_plan() {
        let world = flat_world();
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let intent = ContextualCommandIntent {
            command_type: CommandType::Stop,
            target: CommandTarget::Terrain { position: pos(0.0, 0.0) },
        };
        assert_eq!(
            build_command_plan(&intent, &selection, &world).unwrap(),
            BuiltCommandPlan::StopAll
        );
    }

    #[test]
    fn unknown_target_unit_falls_back_via_or_fallback() {
        let world = flat_world();
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let intent = ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Unit {
                unit_id: crate::world::UnitId::new(999),
            },
        };
        let plan = build_command_plan_or_fallback_move(
            &intent,
            &selection,
            &world,
            Some(pos(1.0, 1.0)),
        );
        assert_eq!(plan, BuiltCommandPlan::MoveTo { target: pos(1.0, 1.0) });
    }

    #[test]
    fn multi_unit_selection_builds_single_batch_plan() {
        let world = flat_world();
        let mut selection = SelectedUnits::default();
        selection.replace_with([
            crate::world::UnitId::new(1),
            crate::world::UnitId::new(2),
        ]);
        let intent = ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Terrain { position: pos(40.0, 40.0) },
        };
        let plan = build_command_plan(&intent, &selection, &world).unwrap();
        assert!(matches!(plan, BuiltCommandPlan::MoveTo { .. }));
    }
}
