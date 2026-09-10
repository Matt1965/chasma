//! Shared field-requirement evaluation for simulation and UI (ADR-104 TF4).

use crate::world::BasisPoints;
use crate::world::TerrainFieldId;
use crate::world::building::field_requirement::BuildingFieldRequirementDefinition;
use crate::world::building::field_response::field_value_to_percent_display;

use super::types::{BuildingFieldRequirementAssessment, RequirementAssessmentAvailability};

/// Authoritative per-field evaluation shared by production gates and player diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingFieldRequirementEvaluation {
    pub field_id: TerrainFieldId,
    pub sampled_average: Option<u16>,
    pub display_average_percent: Option<f32>,
    pub usable_coverage_basis_points: BasisPoints,
    pub minimum_average: u16,
    pub minimum_average_percent_display: f32,
    pub minimum_coverage_basis_points: u16,
    pub average_requirement_met: bool,
    pub coverage_requirement_met: bool,
    pub can_operate: bool,
    pub availability: RequirementAssessmentAvailability,
    pub primary_failure: Option<FieldRequirementFailureReason>,
}

/// Ordered failure reason for one field requirement (matches operational gate priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRequirementFailureReason {
    FieldUnavailable,
    AverageBelowMinimum,
    CoverageBelowMinimum,
    ResponseZero,
}

impl FieldRequirementFailureReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::FieldUnavailable => "Terrain field unavailable",
            Self::AverageBelowMinimum => "Terrain average below minimum",
            Self::CoverageBelowMinimum => "Terrain coverage below minimum",
            Self::ResponseZero => "Terrain response zero",
        }
    }
}

/// Build one evaluation from a cached requirement assessment and its authored definition.
pub fn evaluate_field_requirement(
    requirement: &BuildingFieldRequirementDefinition,
    assessment: &BuildingFieldRequirementAssessment,
) -> BuildingFieldRequirementEvaluation {
    let display_average_percent = assessment.average_value.map(field_value_to_percent_display);
    let primary_failure = primary_failure_for_assessment(assessment);
    BuildingFieldRequirementEvaluation {
        field_id: assessment.field_id.clone(),
        sampled_average: assessment.average_value,
        display_average_percent,
        usable_coverage_basis_points: assessment.usable_coverage_basis_points,
        minimum_average: requirement.minimum_average,
        minimum_average_percent_display: field_value_to_percent_display(
            requirement.minimum_average,
        ),
        minimum_coverage_basis_points: requirement.minimum_usable_coverage_basis_points,
        average_requirement_met: assessment.average_requirement_met,
        coverage_requirement_met: assessment.coverage_requirement_met,
        can_operate: assessment.can_operate,
        availability: assessment.availability,
        primary_failure,
    }
}

/// Evaluate from assessment only (threshold labels omitted).
pub fn evaluate_field_requirement_assessment(
    assessment: &BuildingFieldRequirementAssessment,
) -> BuildingFieldRequirementEvaluation {
    BuildingFieldRequirementEvaluation {
        field_id: assessment.field_id.clone(),
        sampled_average: assessment.average_value,
        display_average_percent: assessment.average_value.map(field_value_to_percent_display),
        usable_coverage_basis_points: assessment.usable_coverage_basis_points,
        minimum_average: 0,
        minimum_average_percent_display: 0.0,
        minimum_coverage_basis_points: 0,
        average_requirement_met: assessment.average_requirement_met,
        coverage_requirement_met: assessment.coverage_requirement_met,
        can_operate: assessment.can_operate,
        availability: assessment.availability,
        primary_failure: primary_failure_for_assessment(assessment),
    }
}

pub fn primary_failure_for_assessment(
    assessment: &BuildingFieldRequirementAssessment,
) -> Option<FieldRequirementFailureReason> {
    if assessment.availability != RequirementAssessmentAvailability::Available {
        return Some(FieldRequirementFailureReason::FieldUnavailable);
    }
    if !assessment.coverage_requirement_met {
        return Some(FieldRequirementFailureReason::CoverageBelowMinimum);
    }
    if !assessment.average_requirement_met {
        return Some(FieldRequirementFailureReason::AverageBelowMinimum);
    }
    if assessment.response_efficiency_basis_points.value() == 0 {
        return Some(FieldRequirementFailureReason::ResponseZero);
    }
    None
}

/// Player-facing single-line field diagnostic from authoritative evaluation.
pub fn format_field_requirement_diagnostic(
    evaluation: &BuildingFieldRequirementEvaluation,
) -> String {
    let field = evaluation.field_id.as_str();
    let average = evaluation
        .display_average_percent
        .map(|percent| format!("{percent:.0}%"))
        .unwrap_or_else(|| "Unknown".to_string());
    let coverage = format!(
        "{:.0}%",
        evaluation.usable_coverage_basis_points.as_percent_display()
    );
    if evaluation.minimum_average_percent_display > 0.0 {
        format!(
            "{field}: {average} (min {:.0}%) | Coverage {coverage}",
            evaluation.minimum_average_percent_display
        )
    } else {
        format!("{field}: {average} | Coverage {coverage}")
    }
}
