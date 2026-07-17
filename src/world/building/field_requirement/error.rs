use super::definition::BuildingFieldRequirementDefinition;
use crate::world::building::field_response::FieldResponseProfileId;
use crate::world::{BuildingDefinitionId, FootprintId, TerrainFieldId};

/// Requirement catalog validation errors (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingFieldRequirementError {
    DuplicateRequirement {
        building_id: BuildingDefinitionId,
        field_id: TerrainFieldId,
    },
    PrimaryOverlayConflict(BuildingDefinitionId),
    InvalidCoverageRequirement {
        building_id: BuildingDefinitionId,
        field_id: TerrainFieldId,
        coverage_basis_points: u16,
    },
    MissingBuilding(BuildingDefinitionId),
    MissingField(TerrainFieldId),
    MissingResponseProfile(FieldResponseProfileId),
    DisabledResponseProfile(FieldResponseProfileId),
    DisabledField(TerrainFieldId),
    SamplingFootprintMissing(FootprintId),
    RonIo(String),
    RonParse(String),
}

impl std::fmt::Display for BuildingFieldRequirementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateRequirement {
                building_id,
                field_id,
            } => write!(
                f,
                "duplicate requirement for building `{building_id}` field `{field_id}`"
            ),
            Self::PrimaryOverlayConflict(id) => {
                write!(f, "multiple primary overlay fields for building `{id}`")
            }
            Self::InvalidCoverageRequirement {
                building_id,
                field_id,
                coverage_basis_points,
            } => write!(
                f,
                "invalid coverage {coverage_basis_points} bp for building `{building_id}` field `{field_id}`"
            ),
            Self::MissingBuilding(id) => write!(f, "building `{id}` missing for requirement"),
            Self::MissingField(id) => write!(f, "terrain field `{id}` missing for requirement"),
            Self::MissingResponseProfile(id) => write!(f, "response profile `{id}` missing"),
            Self::DisabledResponseProfile(id) => write!(f, "response profile `{id}` disabled"),
            Self::DisabledField(id) => write!(f, "terrain field `{id}` disabled"),
            Self::SamplingFootprintMissing(id) => {
                write!(f, "sampling footprint `{}` missing", id.as_str())
            }
            Self::RonIo(msg) => write!(f, "requirement catalog io error: {msg}"),
            Self::RonParse(msg) => write!(f, "requirement catalog parse error: {msg}"),
        }
    }
}

/// Assessment-time requirement errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingFieldRequirementAssessmentError {
    BuildingDefinitionMissing(BuildingDefinitionId),
    RequirementMissing {
        building_id: BuildingDefinitionId,
        field_id: TerrainFieldId,
    },
    OperationalFootprintUnavailable(String),
    SamplingRegionEmpty,
}

impl std::fmt::Display for BuildingFieldRequirementAssessmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildingDefinitionMissing(id) => write!(f, "building definition `{id}` missing"),
            Self::RequirementMissing {
                building_id,
                field_id,
            } => write!(
                f,
                "requirement missing for building `{building_id}` field `{field_id}`"
            ),
            Self::OperationalFootprintUnavailable(msg) => {
                write!(f, "operational footprint unavailable: {msg}")
            }
            Self::SamplingRegionEmpty => write!(f, "sampling region empty"),
        }
    }
}

/// Validate one requirement against referenced catalogs.
pub fn validate_requirement(
    requirement: &BuildingFieldRequirementDefinition,
    building_exists: bool,
    field_exists: bool,
    field_enabled: bool,
    profile_exists: bool,
    profile_enabled: bool,
    footprint_exists: bool,
) -> Result<(), BuildingFieldRequirementError> {
    if !building_exists {
        return Err(BuildingFieldRequirementError::MissingBuilding(
            requirement.building_definition_id.clone(),
        ));
    }
    if !field_exists {
        return Err(BuildingFieldRequirementError::MissingField(
            requirement.terrain_field_id.clone(),
        ));
    }
    if !field_enabled {
        return Err(BuildingFieldRequirementError::DisabledField(
            requirement.terrain_field_id.clone(),
        ));
    }
    if !profile_exists {
        return Err(BuildingFieldRequirementError::MissingResponseProfile(
            requirement.response_profile_id.clone(),
        ));
    }
    if !profile_enabled {
        return Err(BuildingFieldRequirementError::DisabledResponseProfile(
            requirement.response_profile_id.clone(),
        ));
    }
    if requirement.minimum_usable_coverage_basis_points > 10_000 {
        return Err(BuildingFieldRequirementError::InvalidCoverageRequirement {
            building_id: requirement.building_definition_id.clone(),
            field_id: requirement.terrain_field_id.clone(),
            coverage_basis_points: requirement.minimum_usable_coverage_basis_points,
        });
    }
    if let Some(footprint_id) = &requirement.sampling_footprint_id {
        if !footprint_exists {
            return Err(BuildingFieldRequirementError::SamplingFootprintMissing(
                footprint_id.clone(),
            ));
        }
    }
    Ok(())
}
