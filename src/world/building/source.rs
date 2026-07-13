use bevy::prelude::*;

/// How a building entered the world (ADR-079 B2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum BuildingSource {
    Authored,
    /// Runtime placement via dev authoring tools (ADR-043).
    Dev,
}
