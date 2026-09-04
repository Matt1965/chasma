//! Authoritative workforce permission read/write API.

use std::fmt;

use crate::world::{SettlementId, UnitId, WorldData};
use crate::world::{settlement_for_building_work, unit_is_settlement_member};

use super::domain::WorkPermissionDomain;
use super::mapping::work_permission_domain_for_task;
use super::store::WorkforcePermissionStore;
use crate::world::task::TaskType;
use crate::world::{BuildingCatalog, BuildingId, OperationCatalog};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkforcePermissionError {
    SettlementNotFound(SettlementId),
    UnitNotFound(UnitId),
    UnitNotSettlementMember {
        unit_id: UnitId,
        settlement_id: SettlementId,
    },
}

impl fmt::Display for WorkforcePermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SettlementNotFound(id) => write!(f, "settlement {id:?} not found"),
            Self::UnitNotFound(id) => write!(f, "unit {id:?} not found"),
            Self::UnitNotSettlementMember {
                unit_id,
                settlement_id,
            } => write!(
                f,
                "unit {unit_id:?} is not a member of settlement {settlement_id:?}"
            ),
        }
    }
}

impl std::error::Error for WorkforcePermissionError {}

pub fn workforce_permission_store(world: &WorldData) -> &WorkforcePermissionStore {
    world.settlement_store().workforce_permissions()
}

pub fn workforce_permission_store_mut(world: &mut WorldData) -> &mut WorkforcePermissionStore {
    world.settlement_store_mut().workforce_permissions_mut()
}

/// Whether `unit_id` may be considered for autonomous `domain` work in `settlement_id`.
///
/// Absent deny entries default to **allowed** (preserves legacy behavior).
pub fn unit_work_allowed(
    world: &WorldData,
    settlement_id: SettlementId,
    unit_id: UnitId,
    domain: WorkPermissionDomain,
) -> bool {
    world
        .settlement_store()
        .workforce_permissions()
        .is_allowed(settlement_id, unit_id, domain)
}

pub fn set_unit_work_permission(
    world: &mut WorldData,
    settlement_id: SettlementId,
    unit_id: UnitId,
    domain: WorkPermissionDomain,
    allowed: bool,
) -> Result<(), WorkforcePermissionError> {
    if world
        .settlement_store()
        .get_settlement(settlement_id)
        .is_none()
    {
        return Err(WorkforcePermissionError::SettlementNotFound(settlement_id));
    }
    if world.get_unit(unit_id).is_none() {
        return Err(WorkforcePermissionError::UnitNotFound(unit_id));
    }
    if !unit_is_settlement_member(world, unit_id, settlement_id) {
        return Err(WorkforcePermissionError::UnitNotSettlementMember {
            unit_id,
            settlement_id,
        });
    }
    world
        .settlement_store_mut()
        .workforce_permissions_mut()
        .set_allowed(settlement_id, unit_id, domain, allowed);
    Ok(())
}

pub fn clear_unit_workforce_permissions(world: &mut WorldData, unit_id: UnitId) {
    world
        .settlement_store_mut()
        .workforce_permissions_mut()
        .clear_unit(unit_id);
}

pub fn clear_settlement_workforce_permissions(world: &mut WorldData, settlement_id: SettlementId) {
    world
        .settlement_store_mut()
        .workforce_permissions_mut()
        .clear_settlement(settlement_id);
}

/// Autonomous settlement work permission gate (third layer after capability + membership).
pub fn unit_may_autonomously_perform_work(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    unit_id: UnitId,
    building_id: BuildingId,
    task_type: TaskType,
) -> bool {
    let domain = work_permission_domain_for_task(
        world,
        building_catalog,
        operation_catalog,
        building_id,
        task_type,
    );
    match domain {
        None => true,
        Some(domain) => match settlement_for_building_work(world, building_id) {
            None => true,
            Some(settlement_id) => unit_work_allowed(world, settlement_id, unit_id, domain),
        },
    }
}
