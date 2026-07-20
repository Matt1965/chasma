use bevy::prelude::*;

use super::types::TaskType;
use crate::world::unit::UnitWorkCapabilities;
use crate::world::{
    BuildingId, BuildingLifecycleState, BuildingOwnership, BuildingRecord, UnitCatalog, UnitId,
    UnitOwnership, WorldData,
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
        TaskType::Haul => caps.can_operate_workstation,
        // Strategic kinds stay Available until future assignment/execution phases (SA7+).
        TaskType::StrategicConstruct
        | TaskType::RepairBuilding
        | TaskType::ClearRubble
        | TaskType::RecruitWorker
        | TaskType::ExpandStorage => false,
    }
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
