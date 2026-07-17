use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// How a building relates to a terrain field requirement (ADR-104 TF4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum BuildingFieldRequirementKind {
    RequiredEfficiency,
    /// Future seam — not combined in TF4.
    OptionalBonus,
}

/// One building-to-field requirement row (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BuildingFieldRequirementDefinition {
    pub building_definition_id: crate::world::BuildingDefinitionId,
    pub terrain_field_id: crate::world::TerrainFieldId,
    pub requirement_kind: BuildingFieldRequirementKind,
    pub response_profile_id: crate::world::building::field_response::FieldResponseProfileId,
    pub minimum_average: u16,
    pub minimum_usable_coverage_basis_points: u16,
    pub usable_value_threshold: u16,
    pub sampling_footprint_id: Option<crate::world::FootprintId>,
    pub primary_overlay: bool,
    pub overlay_priority: u32,
    pub enabled: bool,
}

impl BuildingFieldRequirementDefinition {
    pub fn sort_key(&self) -> (String, String, u32) {
        (
            self.building_definition_id.as_str().to_string(),
            self.terrain_field_id.as_str().to_string(),
            self.overlay_priority,
        )
    }
}
