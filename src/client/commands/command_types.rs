//! Extended RTS command model (ADR-041 U-UI5).
//!
//! Pure data — classification only; execution lives in [`super::command_builder`].

use bevy::prelude::*;

use crate::world::{UnitId, WorldPosition};

/// High-level command semantics (SC2-style command palette foundation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum CommandType {
    #[default]
    Move,
    Stop,
    HoldPosition,
    /// Placeholder — resolves to move until combat exists.
    AttackMove,
    /// Placeholder — future worker / interact hook.
    Interact,
}

impl CommandType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Stop => "Stop",
            Self::HoldPosition => "Hold Position",
            Self::AttackMove => "Attack Move",
            Self::Interact => "Interact",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Move => "Move selected units to target",
            Self::Stop => "Stop selected units",
            Self::HoldPosition => "Hold position (placeholder)",
            Self::AttackMove => "Attack-move (placeholder — moves for now)",
            Self::Interact => "Interact (placeholder)",
        }
    }

    /// Whether U-UI5 executes real simulation effects for this command.
    pub fn is_fully_functional(self) -> bool {
        matches!(self, Self::Move | Self::Stop)
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
    fn only_move_and_stop_are_functional_in_u5() {
        assert!(CommandType::Move.is_fully_functional());
        assert!(CommandType::Stop.is_fully_functional());
        assert!(!CommandType::HoldPosition.is_fully_functional());
        assert!(!CommandType::AttackMove.is_fully_functional());
    }

    #[test]
    fn contextual_intent_equality() {
        let a = ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Terrain { position: pos(1.0, 2.0) },
        };
        let b = a.clone();
        assert_eq!(a, b);
    }
}
