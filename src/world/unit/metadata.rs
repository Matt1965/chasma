use bevy::prelude::*;

/// Optional per-unit metadata container (ADR-027 U2).
///
/// Intentionally empty in U2. Future phases may add spawn tags, quest hooks,
/// or custom author data without changing [`super::record::UnitRecord`] identity
/// fields. Dynamic ownership (`OwnerId`, `TeamId`, `AffiliationId`) will live
/// here or on simulation state — not copied from catalog `faction_tag`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub struct UnitMetadata;
