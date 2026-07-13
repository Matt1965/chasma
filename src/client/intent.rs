//! Client intent model (ADR-038 U-UI2).
//!
//! Intents are pure data describing player actions before command issuance.

use bevy::prelude::*;

use crate::world::{UnitId, WorldPosition};

/// A single client-side player action awaiting dispatch.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub enum ClientIntent {
    /// Replace selection with one unit (left-click unit, no shift).
    SelectUnit { unit_id: UnitId },
    /// Shift-click toggle unit membership.
    ToggleUnitSelection { unit_id: UnitId },
    /// Marquee select — replace selection with units in the screen rect.
    BoxSelect { rect_min: Vec2, rect_max: Vec2 },
    /// Marquee select — add units in the screen rect to selection.
    BoxSelectAdd { rect_min: Vec2, rect_max: Vec2 },
    /// Clear the local selection (left-click terrain, no shift).
    ClearSelection,
    /// Context-aware command from right-click (terrain or unit target).
    ContextualCommand {
        target: super::commands::CommandTarget,
    },
    /// Legacy move intent — routed through the contextual command pipeline.
    MoveCommand { target: WorldPosition },
    /// Explicit command palette selection (Stop / Hold / future hotkeys).
    PaletteCommand {
        command_type: crate::client::commands::CommandType,
    },
    /// Shift modifier edge (optional; also tracked on [`ClientInputModifiers`]).
    ShiftModifier { pressed: bool },
    /// Enter player build mode (B4).
    EnterBuildMode,
    /// Exit player build mode (B4).
    ExitBuildMode,
    /// Cancel armed ghost or close catalog (B4).
    CancelBuildPlacement,
    /// Rotate armed ghost by 90° (B4).
    RotateBuildGhost,
    /// Arm a building definition for placement (B4).
    SelectBuildingDefinition {
        definition_id: crate::world::BuildingDefinitionId,
    },
    /// Commit a validated building placement (B4).
    PlaceBuilding {
        definition_id: crate::world::BuildingDefinitionId,
        anchor: WorldPosition,
        rotation: Quat,
    },
}
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct ClientIntentQueue {
    pending: Vec<ClientIntent>,
}

impl ClientIntentQueue {
    pub fn push(&mut self, intent: ClientIntent) {
        self.pending.push(intent);
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn pending(&self) -> &[ClientIntent] {
        &self.pending
    }

    /// Remove all pending intents (after dispatch).
    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// Take pending intents in FIFO order.
    pub fn drain(&mut self) -> Vec<ClientIntent> {
        std::mem::take(&mut self.pending)
    }
}

/// Stateful modifier keys sampled before intent collection each frame.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientInputModifiers {
    pub shift: bool,
    /// Selectable unit filter for this frame (ADR-051 O1).
    pub selection_policy: crate::world::SelectionControllabilityPolicy,
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
    fn queue_stores_intents_in_order() {
        let mut queue = ClientIntentQueue::default();
        queue.push(ClientIntent::SelectUnit {
            unit_id: UnitId::new(1),
        });
        queue.push(ClientIntent::MoveCommand {
            target: pos(10.0, 10.0),
        });
        assert_eq!(queue.len(), 2);
        assert_eq!(
            queue.pending()[0],
            ClientIntent::SelectUnit {
                unit_id: UnitId::new(1),
            }
        );
    }

    #[test]
    fn queue_clears_after_drain() {
        let mut queue = ClientIntentQueue::default();
        queue.push(ClientIntent::ClearSelection);
        let drained = queue.drain();
        assert_eq!(drained.len(), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn shift_modifier_intent_updates_state() {
        let intent = ClientIntent::ShiftModifier { pressed: true };
        assert_eq!(intent, ClientIntent::ShiftModifier { pressed: true });
    }
}
