use bevy::prelude::*;

use super::catalog::BuildingDefinitionId;
use super::id::BuildingId;
use super::ownership::BuildingOwnership;
use super::placement::BuildingPlacement;
use super::source::BuildingSource;
use super::state::{
    BuildingInteriorState, BuildingLifecycleState, BuildingSpaces, ConstructionState,
};
use super::vitals::BuildingVitals;

/// One authoritative building instance (ADR-079 B2, ADR-082 B5).
///
/// Type metadata lives in [`BuildingCatalog`]; this record stores runtime state only.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingRecord {
    pub id: BuildingId,
    pub definition_id: BuildingDefinitionId,
    pub placement: BuildingPlacement,
    pub ownership: BuildingOwnership,
    pub vitals: BuildingVitals,
    pub lifecycle_state: BuildingLifecycleState,
    pub spaces: BuildingSpaces,
    pub interior: BuildingInteriorState,
    pub construction: ConstructionState,
    pub source: BuildingSource,
    /// Parent building when this record is an interior child object (ADR-084 B7).
    pub parent_building_id: Option<BuildingId>,
}

impl BuildingRecord {
    pub fn new(
        id: BuildingId,
        definition_id: BuildingDefinitionId,
        placement: BuildingPlacement,
        ownership: BuildingOwnership,
        max_hp: u32,
        source: BuildingSource,
    ) -> Self {
        Self {
            id,
            definition_id,
            placement,
            ownership,
            vitals: BuildingVitals::full(max_hp),
            lifecycle_state: BuildingLifecycleState::Complete,
            spaces: BuildingSpaces::default(),
            interior: BuildingInteriorState::default(),
            construction: ConstructionState::default(),
            source,
            parent_building_id: None,
        }
    }
}
