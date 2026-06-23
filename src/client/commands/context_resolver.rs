//! Context resolution — classifies clicks into [`ContextualCommandIntent`] (ADR-041 U-UI5).

use bevy::prelude::Vec3;

use crate::world::{UnitId, WorldPosition};

use super::command_types::{CommandTarget, CommandType, ContextualCommandIntent};

/// Inputs available when resolving a right-click into a contextual command.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandResolutionContext<'a> {
    pub selected_units: &'a [UnitId],
    pub target: CommandTarget,
}

/// Classify a command target given the current selection.
///
/// Returns `None` when the click cannot produce a command (empty selection).
pub fn resolve_contextual_command(
    ctx: &CommandResolutionContext<'_>,
) -> Option<ContextualCommandIntent> {
    if ctx.selected_units.is_empty() {
        return None;
    }

    match &ctx.target {
        CommandTarget::Terrain { position } => Some(ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Terrain { position: *position },
        }),
        CommandTarget::Unit { unit_id } => {
            // Future: AttackMove or Interact based on unit relationship.
            // U-UI5 default fallback — still classified as Move.
            Some(ContextualCommandIntent {
                command_type: CommandType::Move,
                target: CommandTarget::Unit { unit_id: *unit_id },
            })
        }
    }
}

/// Resolve an explicit palette command (keyboard/UI hotkey hook).
pub fn resolve_palette_command(
    command_type: CommandType,
    selected_units: &[UnitId],
    target: Option<CommandTarget>,
) -> Option<ContextualCommandIntent> {
    if selected_units.is_empty() {
        return None;
    }

    let target = target.unwrap_or(CommandTarget::Terrain {
        position: WorldPosition::new(
            crate::world::ChunkCoord::new(0, 0),
            crate::world::LocalPosition::new(Vec3::ZERO),
        ),
    });

    Some(ContextualCommandIntent { command_type, target })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition, UnitId, WorldPosition};
    use bevy::prelude::Vec3;

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn ctx(units: &[UnitId], target: CommandTarget) -> CommandResolutionContext<'_> {
        CommandResolutionContext {
            selected_units: units,
            target,
        }
    }

    #[test]
    fn terrain_click_resolves_to_move() {
        let units = [UnitId::new(1)];
        let resolved = resolve_contextual_command(&ctx(
            &units,
            CommandTarget::Terrain { position: pos(10.0, 10.0) },
        ))
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::Move);
        assert!(matches!(resolved.target, CommandTarget::Terrain { .. }));
    }

    #[test]
    fn unit_click_resolves_to_move_fallback() {
        let units = [UnitId::new(1)];
        let target_unit = UnitId::new(9);
        let resolved = resolve_contextual_command(&ctx(
            &units,
            CommandTarget::Unit {
                unit_id: target_unit,
            },
        ))
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::Move);
        assert_eq!(
            resolved.target,
            CommandTarget::Unit {
                unit_id: target_unit
            }
        );
    }

    #[test]
    fn empty_selection_returns_none() {
        assert!(resolve_contextual_command(&ctx(
            &[],
            CommandTarget::Terrain { position: pos(0.0, 0.0) },
        ))
        .is_none());
    }

    #[test]
    fn resolver_is_deterministic() {
        let units = [UnitId::new(2), UnitId::new(5)];
        let target = CommandTarget::Terrain { position: pos(3.0, 4.0) };
        let a = resolve_contextual_command(&ctx(&units, target.clone()));
        let b = resolve_contextual_command(&ctx(&units, target));
        assert_eq!(a, b);
    }

    #[test]
    fn multi_unit_selection_still_resolves_move() {
        let units = [UnitId::new(1), UnitId::new(2), UnitId::new(3)];
        let resolved = resolve_contextual_command(&ctx(
            &units,
            CommandTarget::Terrain { position: pos(1.0, 1.0) },
        ))
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::Move);
    }
}
