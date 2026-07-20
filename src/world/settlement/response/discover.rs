//! Response discovery — NeedSnapshot → catalog → CandidateResponse (SA3).
//!
//! Read-only. Never executes responses or mutates world state.

use crate::world::building::catalog::BuildingCatalog;
use crate::world::operation::OperationDefinitionId;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::needs::{NeedCatalog, NeedSnapshot, SettlementNeedEvaluation};
use crate::world::settlement::state::SettlementState;
use crate::world::settlement::SettlementId;
use crate::world::{BuildingId, BuildingLifecycleState, WorldData};

use super::candidate::{
    CandidateResponse, ResponseAvailability, ResponseBlockingReason, SettlementResponseCandidates,
};
use super::catalog::ResponseCatalog;
use super::definition::{CapabilityRequirement, ResponseDefinition};
use super::score::score_candidate;

/// Read-only discovery context.
pub struct ResponseDiscoveryContext<'a> {
    pub world: &'a WorldData,
    pub building_catalog: &'a BuildingCatalog,
    pub need_catalog: &'a NeedCatalog,
    pub response_catalog: &'a ResponseCatalog,
    pub emergency_catalog: &'a EmergencyCatalog,
    pub settlement_id: SettlementId,
    pub state: &'a SettlementState,
    pub need_evaluation: &'a SettlementNeedEvaluation,
    pub simulation_tick: u64,
}

/// Discover and score candidate responses for one settlement from its need snapshots.
pub fn discover_settlement_responses(
    ctx: &ResponseDiscoveryContext<'_>,
) -> SettlementResponseCandidates {
    let mut result = SettlementResponseCandidates {
        settlement_id: ctx.settlement_id,
        evaluated_tick: ctx.simulation_tick,
        source_need_tick: ctx.need_evaluation.evaluated_tick,
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };

    for snapshot in &ctx.need_evaluation.snapshots {
        if ctx.need_catalog.get(&snapshot.need_id).is_none() {
            result.diagnostics.push(format!(
                "unknown need `{}` in snapshot — skipped",
                snapshot.need_id.as_str()
            ));
            continue;
        }

        let definitions = ctx
            .response_catalog
            .definitions_for_need(&snapshot.need_id);
        if definitions.is_empty() {
            result.diagnostics.push(format!(
                "no catalog responses for need `{}`",
                snapshot.need_id.as_str()
            ));
            continue;
        }

        for definition in definitions {
            result
                .candidates
                .push(build_candidate(ctx, snapshot, definition));
        }
    }

    // Stable ordering for determinism: need id, then score desc, then response id.
    result.candidates.sort_by(|a, b| {
        a.need_id
            .as_str()
            .cmp(b.need_id.as_str())
            .then_with(|| {
                b.priority_score
                    .partial_cmp(&a.priority_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.response_id.as_str().cmp(b.response_id.as_str()))
    });

    result
}

fn build_candidate(
    ctx: &ResponseDiscoveryContext<'_>,
    snapshot: &NeedSnapshot,
    definition: &ResponseDefinition,
) -> CandidateResponse {
    let mut diagnostics = Vec::new();
    let (availability, blocking, supporting) =
        evaluate_availability(ctx, definition, snapshot, &mut diagnostics);
    let available = availability.is_available();
    let priority_score = score_candidate(
        definition,
        snapshot,
        ctx.state,
        ctx.emergency_catalog,
        available,
    );

    CandidateResponse {
        response_id: definition.id.clone(),
        need_id: snapshot.need_id.clone(),
        response_type: definition.response_type,
        expected_impact: definition.expected_effect.pressure_relief,
        estimated_cost: definition.expected_effect.estimated_cost,
        availability,
        blocking_reason: blocking,
        priority_score,
        supporting_buildings: supporting,
        diagnostics,
    }
}

fn evaluate_availability(
    ctx: &ResponseDiscoveryContext<'_>,
    definition: &ResponseDefinition,
    snapshot: &NeedSnapshot,
    diagnostics: &mut Vec<String>,
) -> (
    ResponseAvailability,
    Option<ResponseBlockingReason>,
    Vec<BuildingId>,
) {
    if !definition.enabled {
        return (
            ResponseAvailability::Unavailable,
            Some(ResponseBlockingReason::DefinitionDisabled),
            Vec::new(),
        );
    }
    if snapshot.pressure == 0 {
        return (
            ResponseAvailability::Unavailable,
            Some(ResponseBlockingReason::ZeroPressure),
            Vec::new(),
        );
    }

    for prereq in &definition.prerequisite_response_ids {
        // SA3 does not track response execution history — prerequisites are catalog structure only.
        // Unmet until a future phase records applied responses.
        diagnostics.push(format!(
            "prerequisite `{}` not tracked in SA3",
            prereq.as_str()
        ));
        return (
            ResponseAvailability::Unavailable,
            Some(ResponseBlockingReason::PrerequisiteUnmet(
                prereq.as_str().to_string(),
            )),
            Vec::new(),
        );
    }

    let mut supporting = Vec::new();
    for req in &definition.capability_requirements {
        match check_capability(ctx, req, &mut supporting, diagnostics) {
            Ok(()) => {}
            Err(reason) => {
                return (ResponseAvailability::Unavailable, Some(reason), supporting);
            }
        }
    }

    // SA8: emergency unlock / block (authored; not hardcoded by emergency name).
    if let Err(detail) = crate::world::settlement::emergency::emergency_only_gate(
        ctx.state,
        ctx.emergency_catalog,
        definition,
    ) {
        return (
            ResponseAvailability::Unavailable,
            Some(ResponseBlockingReason::Emergency(detail)),
            supporting,
        );
    }
    if let Some(detail) = crate::world::settlement::emergency::emergency_blocks_response(
        ctx.state,
        ctx.emergency_catalog,
        definition,
    ) {
        return (
            ResponseAvailability::Unavailable,
            Some(ResponseBlockingReason::Emergency(detail)),
            supporting,
        );
    }

    (ResponseAvailability::Available, None, supporting)
}

fn check_capability(
    ctx: &ResponseDiscoveryContext<'_>,
    req: &CapabilityRequirement,
    supporting: &mut Vec<BuildingId>,
    diagnostics: &mut Vec<String>,
) -> Result<(), ResponseBlockingReason> {
    match req {
        CapabilityRequirement::Always => Ok(()),
        CapabilityRequirement::ExpansionEnabled => {
            if ctx.state.policies.expansion_enabled {
                Ok(())
            } else {
                Err(ResponseBlockingReason::PolicyDisabled(
                    "expansion_enabled".into(),
                ))
            }
        }
        CapabilityRequirement::AutomationEnabled => {
            if ctx.state.policies.automation_enabled {
                Ok(())
            } else {
                Err(ResponseBlockingReason::PolicyDisabled(
                    "automation_enabled".into(),
                ))
            }
        }
        CapabilityRequirement::MinAggression(min) => {
            if ctx.state.policies.aggression >= *min {
                Ok(())
            } else {
                Err(ResponseBlockingReason::PolicyDisabled(format!(
                    "aggression < {min}"
                )))
            }
        }
        CapabilityRequirement::SupportingOperation(op_id) => {
            let op = OperationDefinitionId::new(op_id.clone());
            let buildings = find_supporting_operation_buildings(ctx, &op);
            if buildings.is_empty() {
                diagnostics.push(format!("no building supports operation `{op_id}`"));
                Err(ResponseBlockingReason::MissingCapability(format!(
                    "operation `{op_id}`"
                )))
            } else {
                supporting.extend(buildings);
                Ok(())
            }
        }
        CapabilityRequirement::BuildingDefinition(def_id) => {
            let buildings = find_buildings_with_definition(ctx, def_id);
            if buildings.is_empty() {
                diagnostics.push(format!("no complete building `{def_id}`"));
                Err(ResponseBlockingReason::MissingCapability(format!(
                    "building `{def_id}`"
                )))
            } else {
                supporting.extend(buildings);
                Ok(())
            }
        }
    }
}

fn find_supporting_operation_buildings(
    ctx: &ResponseDiscoveryContext<'_>,
    operation_id: &OperationDefinitionId,
) -> Vec<BuildingId> {
    let mut found = Vec::new();
    let building_ids = ctx
        .world
        .settlement_store()
        .buildings_for_settlement(ctx.settlement_id);
    for building_id in building_ids {
        let Some(record) = ctx.world.get_building(building_id) else {
            continue;
        };
        if record.lifecycle_state != BuildingLifecycleState::Complete {
            continue;
        }
        let Some(definition) = ctx.building_catalog.get(&record.definition_id) else {
            continue;
        };
        if definition
            .supported_operations
            .iter()
            .any(|op| op == operation_id)
        {
            found.push(building_id);
        }
    }
    found
}

fn find_buildings_with_definition(
    ctx: &ResponseDiscoveryContext<'_>,
    definition_id: &str,
) -> Vec<BuildingId> {
    let mut found = Vec::new();
    let building_ids = ctx
        .world
        .settlement_store()
        .buildings_for_settlement(ctx.settlement_id);
    for building_id in building_ids {
        let Some(record) = ctx.world.get_building(building_id) else {
            continue;
        };
        if record.lifecycle_state != BuildingLifecycleState::Complete {
            continue;
        }
        if record.definition_id.as_str() == definition_id {
            found.push(building_id);
        }
    }
    found
}
