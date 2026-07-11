use bevy::prelude::*;

/// How a doodad entered the world (ADR-015).
///
/// Distinguishes designer-authored placements from procedural baseline content.
/// Future persistence treats procedural output as the initial state and
/// gameplay changes as overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum DoodadSource {
    Authored,
    /// Runtime placement via dev authoring tools (ADR-043).
    Dev,
    Procedural {
        seed: u64,
    },
}
