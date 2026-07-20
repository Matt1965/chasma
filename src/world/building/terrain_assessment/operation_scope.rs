//! Operation-scoped terrain efficiency from cached building assessments (EP6).

use crate::world::TerrainFieldId;
use crate::world::building::field_response::{
    EfficiencyBasisPoints, field_value_to_percent_display,
};
use crate::world::operation::OperationDefinition;

use super::types::BuildingTerrainAssessment;

/// Terrain efficiency narrowed to one selected operation's field requirements (EP6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationScopedTerrainEfficiency {
    pub terrain_efficiency_basis_points: EfficiencyBasisPoints,
    pub can_operate: bool,
    pub limiting_field: Option<TerrainFieldId>,
}

/// Derive operation-scoped terrain efficiency from a cached building assessment (EP6).
///
/// Uses the building assessment cache only — never resamples terrain.
pub fn terrain_efficiency_for_operation(
    assessment: &BuildingTerrainAssessment,
    operation: &OperationDefinition,
) -> OperationScopedTerrainEfficiency {
    if operation.terrain_requirements.is_empty() {
        return OperationScopedTerrainEfficiency {
            terrain_efficiency_basis_points: assessment.terrain_efficiency_basis_points,
            can_operate: assessment.can_operate,
            limiting_field: assessment.limiting_field.clone(),
        };
    }

    let mut min_efficiency = EfficiencyBasisPoints::ONE_HUNDRED_PERCENT;
    let mut can_operate = true;
    let mut limiting_field = None;

    for op_req in &operation.terrain_requirements {
        let Some(req_assessment) = assessment
            .per_requirement
            .iter()
            .find(|entry| entry.field_id == op_req.field_id)
        else {
            return OperationScopedTerrainEfficiency {
                terrain_efficiency_basis_points: EfficiencyBasisPoints::ZERO,
                can_operate: false,
                limiting_field: Some(op_req.field_id.clone()),
            };
        };

        if let Some(avg) = req_assessment.average_value {
            let percent = field_value_to_percent_display(avg);
            if percent < f32::from(op_req.minimum_average_percent) {
                can_operate = false;
                if limiting_field.is_none() {
                    limiting_field = Some(op_req.field_id.clone());
                }
            }
        }

        if !req_assessment.can_operate {
            can_operate = false;
            if limiting_field.is_none() {
                limiting_field = Some(op_req.field_id.clone());
            }
        }

        if req_assessment.response_efficiency_basis_points.value() < min_efficiency.value() {
            min_efficiency = req_assessment.response_efficiency_basis_points;
            limiting_field = Some(op_req.field_id.clone());
        }
    }

    if !can_operate {
        min_efficiency = EfficiencyBasisPoints::ZERO;
    }

    OperationScopedTerrainEfficiency {
        terrain_efficiency_basis_points: min_efficiency,
        can_operate,
        limiting_field,
    }
}
