//! Authoritative Building operational-efficiency query (ADR-105 TF5).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::field_response::EfficiencyBasisPoints;
use crate::world::building::state::BuildingLifecycleState;
use crate::world::building::terrain_assessment::{
    BuildingFieldRequirementAssessment, BuildingTerrainAssessmentStore, BuildingTerrainWarning,
    RequirementAssessmentAvailability, TerrainAssessmentCatalogs, assessment_revision_fingerprint,
    ensure_building_terrain_assessment, terrain_efficiency_for_operation,
};
use crate::world::operation::OperationDefinition;
use crate::world::{BuildingId, WorldData};

use super::combine::combine_output_efficiency;
use super::error::OperationalEfficiencyError;
use super::types::{OperationalEfficiencyReport, OperationalLimitingFactor};

/// Bundled catalogs for operational-efficiency queries.
pub struct OperationalEfficiencyContext<'a> {
    pub world: &'a WorldData,
    pub building_catalog: &'a BuildingCatalog,
    pub terrain_catalogs: TerrainAssessmentCatalogs<'a>,
    pub assessment_store: &'a mut BuildingTerrainAssessmentStore,
}

/// One authoritative operational-efficiency query (ADR-105 TF5, EP6 operation scope).
pub fn building_operational_efficiency(
    ctx: &mut OperationalEfficiencyContext<'_>,
    building_id: BuildingId,
    selected_operation: Option<&OperationDefinition>,
) -> Result<OperationalEfficiencyReport, OperationalEfficiencyError> {
    let record = ctx
        .world
        .get_building(building_id)
        .ok_or(OperationalEfficiencyError::BuildingNotFound(building_id))?
        .clone();

    let definition = ctx
        .building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| {
            OperationalEfficiencyError::BuildingDefinitionMissing(record.definition_id.clone())
        })?;

    if !definition.enabled {
        return Ok(blocked_report(
            building_id,
            OperationalLimitingFactor::BuildingDisabled,
            EfficiencyBasisPoints::ZERO,
            0,
        ));
    }

    if record.lifecycle_state.is_terminal_damage_state() || record.vitals.current_hp == 0 {
        return Ok(blocked_report(
            building_id,
            OperationalLimitingFactor::BuildingDestroyed,
            EfficiencyBasisPoints::ZERO,
            0,
        ));
    }

    if record.lifecycle_state != BuildingLifecycleState::Complete {
        return Ok(blocked_report(
            building_id,
            OperationalLimitingFactor::BuildingIncomplete,
            EfficiencyBasisPoints::ZERO,
            0,
        ));
    }

    let has_requirements = !ctx
        .terrain_catalogs
        .requirements
        .active_required_efficiency(&record.definition_id)
        .is_empty();

    let (terrain_efficiency, terrain_can_operate, limiting, reasons, assessment_revision) =
        if has_requirements {
            let assessment = ensure_building_terrain_assessment(
                ctx.world,
                &ctx.terrain_catalogs,
                ctx.assessment_store,
                building_id,
                &record,
            );
            let revision = assessment_revision_fingerprint(&assessment);
            let scoped = selected_operation
                .map(|operation| terrain_efficiency_for_operation(&assessment, operation))
                .unwrap_or_else(|| {
                    crate::world::building::terrain_assessment::OperationScopedTerrainEfficiency {
                        terrain_efficiency_basis_points: assessment.terrain_efficiency_basis_points,
                        can_operate: assessment.can_operate,
                        limiting_field: assessment.limiting_field.clone(),
                    }
                });
            let scoped_assessment = assessment_with_operation_scope(&assessment, &scoped);
            let (limiting, reasons) = limiting_from_assessment(&scoped_assessment);
            (
                scoped.terrain_efficiency_basis_points,
                scoped.can_operate,
                limiting,
                reasons,
                revision,
            )
        } else {
            (
                EfficiencyBasisPoints::ONE_HUNDRED_PERCENT,
                true,
                OperationalLimitingFactor::None,
                vec![OperationalLimitingFactor::None],
                0,
            )
        };

    let worker = EfficiencyBasisPoints::ONE_HUNDRED_PERCENT;
    let condition = EfficiencyBasisPoints::ONE_HUNDRED_PERCENT;
    let other = EfficiencyBasisPoints::ONE_HUNDRED_PERCENT;

    let combined = combine_output_efficiency(terrain_efficiency, worker, condition, other)?;

    let final_output = if terrain_can_operate {
        combined
    } else {
        EfficiencyBasisPoints::ZERO
    };

    let can_operate = terrain_can_operate;

    Ok(OperationalEfficiencyReport {
        building_id,
        can_operate,
        terrain_efficiency_basis_points: terrain_efficiency,
        worker_efficiency_basis_points: worker,
        condition_efficiency_basis_points: condition,
        other_efficiency_basis_points: other,
        final_output_efficiency_basis_points: final_output,
        limiting_factor: limiting,
        reasons,
        assessment_revision,
    })
}

fn assessment_with_operation_scope(
    assessment: &crate::world::building::terrain_assessment::BuildingTerrainAssessment,
    scoped: &crate::world::building::terrain_assessment::OperationScopedTerrainEfficiency,
) -> crate::world::building::terrain_assessment::BuildingTerrainAssessment {
    let mut narrowed = assessment.clone();
    narrowed.terrain_efficiency_basis_points = scoped.terrain_efficiency_basis_points;
    narrowed.can_operate = scoped.can_operate;
    narrowed.limiting_field = scoped.limiting_field.clone();
    narrowed
}

fn blocked_report(
    building_id: BuildingId,
    limiting_factor: OperationalLimitingFactor,
    terrain: EfficiencyBasisPoints,
    assessment_revision: u64,
) -> OperationalEfficiencyReport {
    OperationalEfficiencyReport {
        building_id,
        can_operate: false,
        terrain_efficiency_basis_points: terrain,
        worker_efficiency_basis_points: EfficiencyBasisPoints::ONE_HUNDRED_PERCENT,
        condition_efficiency_basis_points: EfficiencyBasisPoints::ONE_HUNDRED_PERCENT,
        other_efficiency_basis_points: EfficiencyBasisPoints::ONE_HUNDRED_PERCENT,
        final_output_efficiency_basis_points: EfficiencyBasisPoints::ZERO,
        limiting_factor: limiting_factor.clone(),
        reasons: vec![limiting_factor],
        assessment_revision,
    }
}

fn limiting_from_assessment(
    assessment: &crate::world::building::terrain_assessment::BuildingTerrainAssessment,
) -> (OperationalLimitingFactor, Vec<OperationalLimitingFactor>) {
    let mut reasons = Vec::new();
    for requirement in &assessment.per_requirement {
        reasons.extend(limiting_from_requirement(requirement));
    }
    reasons.sort_by(|left, right| factor_rank(left).cmp(&factor_rank(right)));
    reasons.dedup_by(|a, b| a == b);

    let primary = if assessment.can_operate {
        OperationalLimitingFactor::None
    } else {
        reasons
            .first()
            .cloned()
            .unwrap_or(OperationalLimitingFactor::TerrainResponseZero(
                assessment
                    .limiting_field
                    .clone()
                    .unwrap_or_else(|| crate::world::TerrainFieldId::new("unknown")),
            ))
    };

    if reasons.is_empty() && !assessment.can_operate {
        reasons.push(primary.clone());
    }

    (primary, reasons)
}

fn limiting_from_requirement(
    requirement: &BuildingFieldRequirementAssessment,
) -> Vec<OperationalLimitingFactor> {
    let field_id = requirement.field_id.clone();
    let mut factors = Vec::new();
    if requirement.availability != RequirementAssessmentAvailability::Available {
        factors.push(OperationalLimitingFactor::TerrainFieldUnavailable(
            field_id.clone(),
        ));
    }
    if !requirement.average_requirement_met {
        factors.push(OperationalLimitingFactor::TerrainAverageBelowMinimum(
            field_id.clone(),
        ));
    }
    if !requirement.coverage_requirement_met {
        factors.push(OperationalLimitingFactor::TerrainCoverageBelowMinimum(
            field_id.clone(),
        ));
    }
    if requirement.response_efficiency_basis_points.value() == 0 {
        factors.push(OperationalLimitingFactor::TerrainResponseZero(field_id));
    }
    for warning in &requirement.warnings {
        if matches!(warning, BuildingTerrainWarning::DataUnavailable) {
            factors.push(OperationalLimitingFactor::TerrainFieldUnavailable(
                requirement.field_id.clone(),
            ));
        }
    }
    factors
}

fn factor_rank(factor: &OperationalLimitingFactor) -> u8 {
    match factor {
        OperationalLimitingFactor::BuildingDestroyed => 0,
        OperationalLimitingFactor::BuildingIncomplete => 1,
        OperationalLimitingFactor::BuildingDisabled => 2,
        OperationalLimitingFactor::TerrainFieldUnavailable(_) => 3,
        OperationalLimitingFactor::MissingTerrainAssessment => 4,
        OperationalLimitingFactor::StaleTerrainAssessment => 5,
        OperationalLimitingFactor::TerrainAverageBelowMinimum(_) => 6,
        OperationalLimitingFactor::TerrainCoverageBelowMinimum(_) => 7,
        OperationalLimitingFactor::TerrainResponseZero(_) => 8,
        _ => 9,
    }
}
