//! Task semantics → workforce permission domain (autonomous settlement work).

use crate::world::building::operation::is_prispod_farm_definition;
use crate::world::operation::OperationCategory;
use crate::world::task::TaskType;
use crate::world::{BuildingCatalog, BuildingId, OperationCatalog, WorldData};

use super::domain::WorkPermissionDomain;

/// Map an autonomous task to a player-controllable permission domain, if any.
///
/// Returns `None` for unclassified work — callers must preserve prior behavior (allow).
pub fn work_permission_domain_for_task(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
    task_type: TaskType,
) -> Option<WorkPermissionDomain> {
    match task_type {
        TaskType::ConstructBuilding => Some(WorkPermissionDomain::Construction),
        TaskType::Haul => Some(WorkPermissionDomain::GeneralLabor),
        TaskType::OperateWorkstation => work_permission_domain_for_operate_workstation(
            world,
            building_catalog,
            operation_catalog,
            building_id,
        ),
        TaskType::StrategicConstruct
        | TaskType::RepairBuilding
        | TaskType::ClearRubble
        | TaskType::RecruitWorker
        | TaskType::ExpandStorage => None,
    }
}

fn work_permission_domain_for_operate_workstation(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
) -> Option<WorkPermissionDomain> {
    let building = world.get_building(building_id)?;
    let definition = building_catalog.get(&building.definition_id)?;
    if is_prispod_farm_definition(definition) {
        return Some(WorkPermissionDomain::Farming);
    }
    let store = world.building_production_store();
    let policy = store.get_policy(building_id);
    let effective_operation = policy
        .and_then(|p| p.selected_operation.clone())
        .or_else(|| definition.resolved_default_operation());
    let Some(operation_id) = effective_operation else {
        return None;
    };
    let operation = operation_catalog.get(&operation_id)?;
    match operation.category {
        OperationCategory::Agriculture => Some(WorkPermissionDomain::Farming),
        OperationCategory::Extraction => Some(WorkPermissionDomain::GeneralLabor),
        OperationCategory::Processing
        | OperationCategory::Crafting
        | OperationCategory::Research
        | OperationCategory::Medical
        | OperationCategory::Ritual => None,
    }
}
