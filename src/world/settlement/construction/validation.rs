//! Construction plan validation (SA9).

use std::collections::{HashMap, HashSet};

use crate::world::building::catalog::BuildingCatalog;
use crate::world::WorldData;

use super::catalog::ConstructionResponseCatalog;
use super::plan::{ConstructionPlan, ConstructionPlanStatus};
use super::store::ConstructionPlanStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstructionValidationError {
    DuplicatePlanId(u64),
    UnknownSettlement(u64),
    UnknownBuildingDefinition(String),
    InvalidSourceResponse(String),
    OverlappingReservation {
        plan_a: u64,
        plan_b: u64,
        building_id: u64,
    },
    CancelledRetainsReservation(u64),
    CompletedWithoutBuilding(u64),
    DuplicateActiveFulfillment(String),
    InvalidTransition {
        plan_id: u64,
        status: String,
        detail: String,
    },
}

impl std::fmt::Display for ConstructionValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicatePlanId(id) => write!(f, "duplicate ConstructionPlanId {id}"),
            Self::UnknownSettlement(id) => write!(f, "unknown SettlementId {id}"),
            Self::UnknownBuildingDefinition(id) => {
                write!(f, "unknown BuildingDefinitionId `{id}`")
            }
            Self::InvalidSourceResponse(id) => {
                write!(f, "invalid/missing construction mapping for `{id}`")
            }
            Self::OverlappingReservation {
                plan_a,
                plan_b,
                building_id,
            } => write!(
                f,
                "plans {plan_a} and {plan_b} reserve building {building_id}"
            ),
            Self::CancelledRetainsReservation(id) => {
                write!(f, "cancelled plan {id} still holds reservation")
            }
            Self::CompletedWithoutBuilding(id) => {
                write!(f, "completed plan {id} has no reserved building")
            }
            Self::DuplicateActiveFulfillment(key) => {
                write!(f, "multiple active plans for fulfillment key `{key}`")
            }
            Self::InvalidTransition {
                plan_id,
                status,
                detail,
            } => write!(f, "plan {plan_id} status `{status}`: {detail}"),
        }
    }
}

pub fn validate_construction_plans(
    world: &WorldData,
    plan_store: &ConstructionPlanStore,
    building_catalog: &BuildingCatalog,
    response_catalog: &ConstructionResponseCatalog,
) -> Vec<ConstructionValidationError> {
    let mut errors = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut reservation_owners: HashMap<u64, u64> = HashMap::new();
    let mut fulfillment_owners: HashMap<String, u64> = HashMap::new();

    for plan in plan_store.iter() {
        if !seen_ids.insert(plan.id.raw()) {
            errors.push(ConstructionValidationError::DuplicatePlanId(plan.id.raw()));
        }
        if world
            .settlement_store()
            .get_settlement(plan.settlement_id)
            .is_none()
        {
            errors.push(ConstructionValidationError::UnknownSettlement(
                plan.settlement_id.raw(),
            ));
        }
        if building_catalog.get(&plan.building_definition_id).is_none() {
            errors.push(ConstructionValidationError::UnknownBuildingDefinition(
                plan.building_definition_id.as_str().to_string(),
            ));
        }
        if response_catalog.get(&plan.source.response_id).is_none()
            && plan.source.intent_id.is_some()
        {
            // Manual plans may use synthetic response ids — only warn for intent-sourced.
            errors.push(ConstructionValidationError::InvalidSourceResponse(
                plan.source.response_id.as_str().to_string(),
            ));
        }
        validate_plan_invariants(plan, &mut errors);

        if let Some(building_id) = plan.reserved_building_id {
            if let Some(other) = reservation_owners.insert(building_id.raw(), plan.id.raw()) {
                errors.push(ConstructionValidationError::OverlappingReservation {
                    plan_a: other,
                    plan_b: plan.id.raw(),
                    building_id: building_id.raw(),
                });
            }
        }

        if plan.status.is_active() {
            if let Some(other) = fulfillment_owners.insert(plan.fulfillment_key.clone(), plan.id.raw())
            {
                if other != plan.id.raw() {
                    errors.push(ConstructionValidationError::DuplicateActiveFulfillment(
                        plan.fulfillment_key.clone(),
                    ));
                }
            }
        }
    }

    errors
}

fn validate_plan_invariants(
    plan: &ConstructionPlan,
    errors: &mut Vec<ConstructionValidationError>,
) {
    if plan.status == ConstructionPlanStatus::Cancelled && plan.reserved_building_id.is_some() {
        errors.push(ConstructionValidationError::CancelledRetainsReservation(
            plan.id.raw(),
        ));
    }
    if plan.status == ConstructionPlanStatus::Completed && plan.reserved_building_id.is_none() {
        errors.push(ConstructionValidationError::CompletedWithoutBuilding(
            plan.id.raw(),
        ));
    }
    if plan.status == ConstructionPlanStatus::Ready && plan.reserved_building_id.is_none() {
        errors.push(ConstructionValidationError::InvalidTransition {
            plan_id: plan.id.raw(),
            status: plan.status.as_str().into(),
            detail: "Ready without reserved building".into(),
        });
    }
}

pub fn validate_world_construction_plans(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    response_catalog: &ConstructionResponseCatalog,
) -> Vec<ConstructionValidationError> {
    validate_construction_plans(
        world,
        world.construction_plan_store(),
        building_catalog,
        response_catalog,
    )
}
