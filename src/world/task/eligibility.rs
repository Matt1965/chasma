use bevy::prelude::*;

use super::types::TaskType;
use crate::world::{
    BuildingId, BuildingLifecycleState, BuildingOwnership, BuildingRecord, SettlementId,
    UnitCatalog, UnitId, UnitOwnership, UnitWorkCapabilities, WorldData,
};

pub fn unit_work_capabilities(
    catalog: &UnitCatalog,
    world: &WorldData,
    unit_id: UnitId,
) -> Option<UnitWorkCapabilities> {
    let record = world.get_unit(unit_id)?;
    let definition = catalog.get(&record.definition_id)?;
    Some(definition.work_capabilities)
}

pub fn unit_can_perform_task(
    catalog: &UnitCatalog,
    world: &WorldData,
    unit_id: UnitId,
    task_type: TaskType,
) -> bool {
    let Some(caps) = unit_work_capabilities(catalog, world, unit_id) else {
        return false;
    };
    match task_type {
        TaskType::ConstructBuilding => caps.can_construct,
        TaskType::OperateWorkstation => caps.can_operate_workstation,
        TaskType::Haul => caps.can_haul,
        // Strategic kinds stay Available until future assignment/execution phases (SA7+).
        TaskType::StrategicConstruct
        | TaskType::RepairBuilding
        | TaskType::ClearRubble
        | TaskType::RecruitWorker
        | TaskType::ExpandStorage => false,
    }
}

/// Settlement that owns work at `building_id`, if any (authoritative building field).
pub fn settlement_for_building_work(
    world: &WorldData,
    building_id: BuildingId,
) -> Option<SettlementId> {
    world.get_building(building_id)?.settlement_id
}

pub fn unit_is_settlement_member(
    world: &WorldData,
    unit_id: UnitId,
    settlement_id: SettlementId,
) -> bool {
    world
        .get_unit(unit_id)
        .is_some_and(|unit| unit.settlement_id == Some(settlement_id))
}

/// Autonomous marketplace membership gate (ADR-133 Phase 3).
///
/// Buildings without explicit settlement membership skip this gate; affiliation is
/// checked separately. Settlement-scoped work requires matching `UnitRecord.settlement_id`.
pub fn unit_may_autonomously_work_building(
    world: &WorldData,
    unit_id: UnitId,
    building_id: BuildingId,
) -> bool {
    let Some(building) = world.get_building(building_id) else {
        return false;
    };
    let Some(settlement_id) = building.settlement_id else {
        return true;
    };
    unit_is_settlement_member(world, unit_id, settlement_id)
}

pub fn unit_may_work_on_building(building: &BuildingRecord, unit_ownership: UnitOwnership) -> bool {
    let building_ownership = BuildingOwnership::from_unit_ownership(unit_ownership);
    match (building_ownership.affiliation, unit_ownership.affiliation) {
        (crate::world::Affiliation::Hostile, _) | (_, crate::world::Affiliation::Hostile) => false,
        _ => {
            if building_ownership.owner_id.is_some() && unit_ownership.owner_id.is_some() {
                building_ownership.owner_id == unit_ownership.owner_id
            } else {
                building_ownership.affiliation == unit_ownership.affiliation
            }
        }
    }
}

pub fn building_is_constructible(record: &BuildingRecord) -> bool {
    record.lifecycle_state.receives_construction_progress()
        && !record.lifecycle_state.is_terminal_damage_state()
}

pub fn building_accepts_workstation_use(record: &BuildingRecord) -> bool {
    record.lifecycle_state == BuildingLifecycleState::Complete && record.vitals.current_hp > 0
}

pub fn building_id_from_record_id(id: BuildingId) -> BuildingId {
    id
}

#[cfg(test)]
mod tests;
