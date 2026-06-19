use bevy::prelude::*;

/// Category of environmental world object (ADR-015).
///
/// Extensible enum for authored and procedural content. Gameplay-specific state
/// (harvest depletion, etc.) belongs in [`super::metadata::DoodadMetadata`]
/// later, not in this discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum DoodadKind {
    Tree,
    Rock,
    Bush,
    Ruin,
    ResourceNode,
}
