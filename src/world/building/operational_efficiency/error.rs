use crate::world::BuildingDefinitionId;
use crate::world::BuildingId;
use crate::world::TerrainFieldId;

/// Operational efficiency query failures (ADR-105 TF5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationalEfficiencyError {
    BuildingNotFound(BuildingId),
    BuildingDefinitionMissing(BuildingDefinitionId),
    BuildingNotOperational(BuildingId),
    TerrainAssessmentMissing(BuildingId),
    TerrainAssessmentStale(BuildingId),
    TerrainRequirementUnmet {
        building_id: BuildingId,
        field_id: TerrainFieldId,
    },
    TerrainFieldUnavailable(TerrainFieldId),
    EfficiencyOutOfRange(u32),
    EfficiencyMultiplicationOverflow,
}

impl std::fmt::Display for OperationalEfficiencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildingNotFound(id) => write!(f, "building `{id:?}` not found"),
            Self::BuildingDefinitionMissing(id) => write!(f, "building definition `{id}` missing"),
            Self::BuildingNotOperational(id) => write!(f, "building `{id:?}` not operational"),
            Self::TerrainAssessmentMissing(id) => {
                write!(f, "terrain assessment missing for `{id:?}`")
            }
            Self::TerrainAssessmentStale(id) => write!(f, "terrain assessment stale for `{id:?}`"),
            Self::TerrainRequirementUnmet {
                building_id,
                field_id,
            } => write!(
                f,
                "terrain requirement unmet for building `{building_id:?}` field `{field_id}`"
            ),
            Self::TerrainFieldUnavailable(id) => write!(f, "terrain field `{id}` unavailable"),
            Self::EfficiencyOutOfRange(value) => write!(f, "efficiency {value} bp out of range"),
            Self::EfficiencyMultiplicationOverflow => {
                write!(f, "efficiency multiplication overflow")
            }
        }
    }
}
