use bevy::prelude::*;

use crate::world::BasisPoints;
use crate::world::FieldAreaAvailability;
use crate::world::TerrainFieldId;
use crate::world::building::field_response::{EfficiencyBasisPoints, FieldResponseProfileId};

/// Per-requirement terrain assessment (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingFieldRequirementAssessment {
    pub field_id: TerrainFieldId,
    pub response_profile_id: FieldResponseProfileId,
    pub sample_count: u32,
    pub unavailable_sample_count: u32,
    pub average_value: Option<u16>,
    pub minimum_value: Option<u16>,
    pub maximum_value: Option<u16>,
    pub usable_coverage_basis_points: BasisPoints,
    pub response_efficiency_basis_points: EfficiencyBasisPoints,
    pub average_requirement_met: bool,
    pub coverage_requirement_met: bool,
    pub can_operate: bool,
    pub availability: RequirementAssessmentAvailability,
    pub warnings: Vec<BuildingTerrainWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum RequirementAssessmentAvailability {
    Available,
    PartiallyUnavailable,
    Unavailable,
}

impl RequirementAssessmentAvailability {
    pub fn from_area(area: FieldAreaAvailability, sample_count: u32) -> Self {
        match area {
            FieldAreaAvailability::AllAvailable if sample_count > 0 => Self::Available,
            FieldAreaAvailability::PartiallyAvailable => Self::PartiallyUnavailable,
            _ => Self::Unavailable,
        }
    }
}

/// Combined terrain assessment for one building placement or instance (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingTerrainAssessment {
    pub building_definition_id: crate::world::BuildingDefinitionId,
    pub per_requirement: Vec<BuildingFieldRequirementAssessment>,
    pub terrain_efficiency_basis_points: EfficiencyBasisPoints,
    pub limiting_field: Option<TerrainFieldId>,
    pub can_operate: bool,
    pub sample_footprint_hash: u64,
    pub field_tile_revisions: Vec<FieldTileRevisionEntry>,
    pub requirement_catalog_revision: u64,
    pub profile_catalog_revision: u64,
    pub warnings: Vec<BuildingTerrainWarning>,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct FieldTileRevisionEntry {
    pub field_id: TerrainFieldId,
    pub chunk: crate::world::ChunkCoord,
    pub tile_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum BuildingTerrainWarning {
    PlacementAllowedZeroOutput,
    PlacementAllowedLowEfficiency,
    DataUnavailable,
    AverageBelowMinimum { field_id: TerrainFieldId },
    CoverageBelowMinimum { field_id: TerrainFieldId },
    CannotOperate,
}

impl BuildingTerrainWarning {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PlacementAllowedZeroOutput => "Placement allowed, but output will be zero",
            Self::PlacementAllowedLowEfficiency => {
                "Placement allowed, but terrain efficiency is low"
            }
            Self::DataUnavailable => "Terrain field data unavailable",
            Self::AverageBelowMinimum { .. } => "Average field value below operational minimum",
            Self::CoverageBelowMinimum { .. } => "Usable coverage below operational minimum",
            Self::CannotOperate => "Cannot operate at this location",
        }
    }
}

impl BuildingTerrainAssessment {
    pub fn limiting_reason_label(&self) -> Option<String> {
        if let Some(field_id) = &self.limiting_field {
            if self.terrain_efficiency_basis_points.value() == 0 {
                return Some(format!("Limited by {} (0% output)", field_id.as_str()));
            }
            return Some(format!(
                "Limited by {} ({:.0}% output)",
                field_id.as_str(),
                self.terrain_efficiency_basis_points.as_percent_display()
            ));
        }
        None
    }

    pub fn status_label(&self) -> &'static str {
        if self
            .warnings
            .iter()
            .any(|warning| matches!(warning, BuildingTerrainWarning::DataUnavailable))
        {
            return "Unknown";
        }
        if self.can_operate {
            "Can Operate"
        } else {
            "Cannot Produce"
        }
    }
}
