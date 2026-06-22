use bevy::prelude::*;

use super::catalog::UnitDefinitionId;
use super::id::UnitId;
use super::metadata::UnitMetadata;
use super::placement::UnitPlacement;
use super::source::UnitSource;
use super::state::UnitState;

/// One authoritative unit instance (ADR-027 U2).
///
/// [`UnitDefinitionId`] is the authoritative type reference. Instance records
/// do **not** store faction ownership; catalog `faction_tag` is definition
/// metadata only.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct UnitRecord {
    pub id: UnitId,
    pub definition_id: UnitDefinitionId,
    pub placement: UnitPlacement,
    pub state: UnitState,
    pub source: UnitSource,
    pub metadata: UnitMetadata,
}

impl UnitRecord {
    pub fn new(
        id: UnitId,
        definition_id: UnitDefinitionId,
        placement: UnitPlacement,
        source: UnitSource,
    ) -> Self {
        Self {
            id,
            definition_id,
            placement,
            state: UnitState::default(),
            source,
            metadata: UnitMetadata,
        }
    }
}
