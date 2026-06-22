use bevy::prelude::*;

/// How a unit entered the world (ADR-027 U2).
///
/// Distinguishes designer-authored placements from procedural or scripted spawns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum UnitSource {
    Authored,
    Procedural { seed: u64 },
}
