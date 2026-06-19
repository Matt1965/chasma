use bevy::prelude::*;

/// Optional per-doodad metadata container (ADR-015).
///
/// Intentionally empty in Phase 3A. Future phases may add harvest state,
/// depletion, regrowth, or author tags without changing [`super::record::DoodadRecord`]
/// identity fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub struct DoodadMetadata;
