use bevy::prelude::*;

use super::catalog::DoodadDefinitionId;
use super::id::DoodadId;
use super::kind::DoodadKind;
use super::metadata::DoodadMetadata;
use super::placement::DoodadPlacement;
use super::source::DoodadSource;

/// One authoritative doodad instance (ADR-015, ADR-017).
///
/// [`DoodadDefinitionId`] is the authoritative type reference. [`DoodadKind`] is
/// denormalized from the catalog at creation time for coarse filtering without
/// catalog lookups.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct DoodadRecord {
    pub id: DoodadId,
    pub definition_id: DoodadDefinitionId,
    pub kind: DoodadKind,
    pub placement: DoodadPlacement,
    pub source: DoodadSource,
    pub metadata: DoodadMetadata,
}

impl DoodadRecord {
    pub fn new(
        id: DoodadId,
        definition_id: DoodadDefinitionId,
        kind: DoodadKind,
        placement: DoodadPlacement,
        source: DoodadSource,
    ) -> Self {
        Self {
            id,
            definition_id,
            kind,
            placement,
            source,
            metadata: DoodadMetadata,
        }
    }
}
