//! Task semantics → relevant work skill (performance mapping foundation only).

use crate::world::building::operation::is_prispod_farm_definition;
use crate::world::operation::OperationCategory;
use crate::world::task::TaskType;
use crate::world::{BuildingCatalog, BuildingId, OperationCatalog, WorldData};

use crate::world::WorkPermissionDomain;

use super::id::WorkSkillId;

/// Map a player-controllable workforce permission domain to its displayed work skill.
pub fn work_skill_for_permission_domain(domain: WorkPermissionDomain) -> WorkSkillId {
    match domain {
        WorkPermissionDomain::Farming => WorkSkillId::new("farming"),
        WorkPermissionDomain::GeneralLabor => WorkSkillId::new("general_labor"),
        WorkPermissionDomain::Construction => WorkSkillId::new("construction"),
        WorkPermissionDomain::Cooking => WorkSkillId::new("cooking"),
        WorkPermissionDomain::Science => WorkSkillId::new("science"),
        WorkPermissionDomain::Smithing => WorkSkillId::new("smithing"),
    }
}

/// Map an existing autonomous task to its relevant work skill, if any.
///
/// Does not gate eligibility or performance — informational seam for future systems.
pub fn work_skill_for_task(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
    task_type: TaskType,
) -> Option<WorkSkillId> {
    match task_type {
        TaskType::ConstructBuilding => Some(WorkSkillId::new("construction")),
        TaskType::Haul => Some(WorkSkillId::new("general_labor")),
        TaskType::OperateWorkstation => work_skill_for_operate_workstation(
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

fn work_skill_for_operate_workstation(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
) -> Option<WorkSkillId> {
    let building = world.get_building(building_id)?;
    let definition = building_catalog.get(&building.definition_id)?;
    if is_prispod_farm_definition(definition) {
        return Some(WorkSkillId::new("farming"));
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
        OperationCategory::Agriculture => Some(WorkSkillId::new("farming")),
        OperationCategory::Extraction => Some(WorkSkillId::new("general_labor")),
        OperationCategory::Processing
        | OperationCategory::Crafting
        | OperationCategory::Research
        | OperationCategory::Medical
        | OperationCategory::Ritual => None,
    }
}
