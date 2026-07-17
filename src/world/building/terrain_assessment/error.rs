use crate::world::BuildingDefinitionId;
use crate::world::building::field_requirement::BuildingFieldRequirementCatalog;
use crate::world::building::field_response::{
    FieldResponseEvaluationError, FieldResponseProfileCatalog,
};

/// Terrain assessment errors (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerrainAssessmentError {
    BuildingDefinitionMissing(BuildingDefinitionId),
    BuildingNotFound(crate::world::BuildingId),
    OperationalFootprintUnavailable(String),
    SamplingRegionEmpty,
    TerrainFieldDataUnavailable(crate::world::TerrainFieldId),
    TerrainFieldAreaQueryFailed(String),
    ResponseEvaluationFailed(FieldResponseEvaluationError),
    AssessmentRevisionMismatch,
    AssessmentCacheMissing(crate::world::BuildingId),
    AssessmentInvalidated(crate::world::BuildingId),
}

impl std::fmt::Display for TerrainAssessmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildingDefinitionMissing(id) => write!(f, "building definition `{id}` missing"),
            Self::BuildingNotFound(id) => write!(f, "building `{id:?}` not found"),
            Self::OperationalFootprintUnavailable(msg) => {
                write!(f, "operational footprint unavailable: {msg}")
            }
            Self::SamplingRegionEmpty => write!(f, "sampling region empty"),
            Self::TerrainFieldDataUnavailable(id) => write!(f, "terrain field `{id}` unavailable"),
            Self::TerrainFieldAreaQueryFailed(msg) => {
                write!(f, "terrain field area query failed: {msg}")
            }
            Self::ResponseEvaluationFailed(err) => write!(f, "response evaluation failed: {err}"),
            Self::AssessmentRevisionMismatch => write!(f, "assessment revision mismatch"),
            Self::AssessmentCacheMissing(id) => write!(f, "assessment cache missing for `{id:?}`"),
            Self::AssessmentInvalidated(id) => write!(f, "assessment invalidated for `{id:?}`"),
        }
    }
}

/// UI-facing overlay/assessment presentation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerrainAssessmentUiError {
    TemporaryOverlayFieldMissing(crate::world::TerrainFieldId),
    BuildAssessmentUnavailable,
}

impl std::fmt::Display for TerrainAssessmentUiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TemporaryOverlayFieldMissing(id) => {
                write!(f, "temporary overlay field `{id}` missing")
            }
            Self::BuildAssessmentUnavailable => write!(f, "build assessment unavailable"),
        }
    }
}

/// Bundle of catalogs needed for assessment.
pub struct TerrainAssessmentCatalogs<'a> {
    pub buildings: &'a crate::world::building::catalog::BuildingCatalog,
    pub requirements: &'a BuildingFieldRequirementCatalog,
    pub profiles: &'a FieldResponseProfileCatalog,
    pub fields: &'a crate::world::TerrainFieldCatalog,
    pub footprints: &'a crate::world::FootprintCatalog,
    pub requirement_revision: u64,
    pub profile_revision: u64,
}
