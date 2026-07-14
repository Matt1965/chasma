//! Extended RTS command model (ADR-041 U-UI5, REVIEW-B3).
//!
//! Pure data — classification only; execution lives in [`super::command_builder`].

use bevy::prelude::*;

use crate::world::{UnitId, WorldPosition};

/// High-level command semantics exposed to the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum CommandType {
    #[default]
    Move,
    Stop,
    /// Reserved — not implemented (REVIEW-B3).
    HoldPosition,
    /// Direct attack against a unit target.
    Attack,
    /// Move while scanning for hostile targets (ADR-057).
    AttackMove,
    /// Reserved — worker/interact hook; not player-exposed yet (REVIEW-B3).
    Interact,
}

impl CommandType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Stop => "Stop",
            Self::HoldPosition => "Hold Position",
            Self::Attack => "Attack",
            Self::AttackMove => "Attack Move",
            Self::Interact => "Interact",
        }
    }

    /// Player-facing description (availability suffix added by [`super::command_availability`]).
    pub fn description(self) -> &'static str {
        match self {
            Self::Move => "Move selected units to target",
            Self::Stop => "Stop selected units",
            Self::HoldPosition => "Hold position at current location",
            Self::Attack => "Attack target or attack-move on ground",
            Self::AttackMove => "Attack-move to destination (internal)",
            Self::Interact => "Interact with world object",
        }
    }

    /// Whether the command has a full simulation implementation (REVIEW-B3).
    pub fn is_implemented(self) -> bool {
        matches!(
            self,
            Self::Move | Self::Stop | Self::Attack | Self::AttackMove | Self::Interact
        )
    }

    /// Commands shown in the static player palette.
    pub fn player_palette() -> &'static [CommandType] {
        &[
            CommandType::Move,
            CommandType::Stop,
            CommandType::HoldPosition,
            CommandType::Attack,
        ]
    }
}

/// What the player clicked or targeted when issuing a command.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum CommandTarget {
    Terrain { position: WorldPosition },
    Unit { unit_id: UnitId },
}

/// Resolved command after context classification (intent → context).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ContextualCommandIntent {
    pub command_type: CommandType,
    pub target: CommandTarget,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn command_type_labels_are_stable() {
        assert_eq!(CommandType::Move.label(), "Move");
        assert_eq!(CommandType::AttackMove.label(), "Attack Move");
    }

    #[test]
    fn only_implemented_commands_are_functional() {
        assert!(CommandType::Move.is_implemented());
        assert!(CommandType::Stop.is_implemented());
        assert!(CommandType::Attack.is_implemented());
        assert!(CommandType::AttackMove.is_implemented());
        assert!(!CommandType::HoldPosition.is_implemented());
        assert!(CommandType::Interact.is_implemented());
    }

    #[test]
    fn contextual_intent_equality() {
        let a = ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Terrain {
                position: pos(1.0, 2.0),
            },
        };
        let b = a.clone();
        assert_eq!(a, b);
    }
}
