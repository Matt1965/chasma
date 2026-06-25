//! Interaction classification types (ADR-042 U6).
//!
//! Pure data — no gameplay effects.

use bevy::prelude::*;

use crate::world::{DoodadId, DoodadKind, UnitId, WorldPosition};

/// What the player or AI is interacting with at a world point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum InteractionType {
    #[default]
    None,
    /// Walkable terrain suitable as a move destination.
    MoveTarget,
    /// Hostile or otherwise valid attack target unit.
    AttackableUnit,
    /// Friendly unit — not attackable by default.
    FriendlyUnit,
    /// Neutral unit — not attackable unless weapon filters allow.
    NeutralUnit,
    /// Harvestable resource (read-only stub until U13+).
    ResourceNode,
    /// Non-blocking interactable (doors, markers, ruins).
    InteractableObject,
    /// Movement-blocked area (obstacle footprint or unwalkable terrain).
    BlockedArea,
    /// Grounded terrain sample without a confirmed move/interact target.
    TerrainPoint,
}

impl InteractionType {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::MoveTarget => "MoveTarget",
            Self::AttackableUnit => "AttackableUnit",
            Self::FriendlyUnit => "FriendlyUnit",
            Self::NeutralUnit => "NeutralUnit",
            Self::ResourceNode => "ResourceNode",
            Self::InteractableObject => "InteractableObject",
            Self::BlockedArea => "BlockedArea",
            Self::TerrainPoint => "TerrainPoint",
        }
    }
}

/// Optional authoritative target reference for downstream systems.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum InteractionTargetRef {
    None,
    Doodad(DoodadId),
    Unit(UnitId),
    Terrain(WorldPosition),
}

impl Default for InteractionTargetRef {
    fn default() -> Self {
        Self::None
    }
}

/// Read-only metadata for UI, AI, and debug overlays.
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
pub struct InteractionMetadata {
    pub label: String,
    pub doodad_kind: Option<DoodadKind>,
    pub blocks_movement: bool,
}

/// SC2-style “what is under the cursor?” query result.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct InteractionResult {
    pub interaction_type: InteractionType,
    pub position: WorldPosition,
    pub metadata: InteractionMetadata,
    pub valid: bool,
    pub target: InteractionTargetRef,
}

impl InteractionResult {
    pub fn invalid(position: WorldPosition) -> Self {
        Self {
            interaction_type: InteractionType::None,
            position,
            metadata: InteractionMetadata::default(),
            valid: false,
            target: InteractionTargetRef::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_type_labels_are_stable() {
        assert_eq!(InteractionType::MoveTarget.label(), "MoveTarget");
        assert_eq!(InteractionType::ResourceNode.label(), "ResourceNode");
    }
}
