//! Workstation presentation clip selection (extraction labor → profile work clip).

use crate::world::{
    AnimationClipKey, AnimationProfile, BuildingCatalog, OperationCatalog, OperationCategory,
    TaskType, UnitId, UnitRecord, UnitState, WorldData,
};

/// Catalogs required to map a working unit to its active operation category.
pub struct WorkPresentationContext<'a> {
    pub world: &'a WorldData,
    pub building_catalog: &'a BuildingCatalog,
    pub operation_catalog: &'a OperationCatalog,
}

/// When a unit is applying extraction labor, return the profile work clip key.
pub fn working_locomotion_clip(
    record: &UnitRecord,
    profile: &AnimationProfile,
    ctx: Option<&WorkPresentationContext<'_>>,
) -> Option<AnimationClipKey> {
    if profile.resolve_work_clip_name().is_none() {
        return None;
    }
    let UnitState::Working { task_id } = &record.state else {
        return None;
    };
    let ctx = ctx?;
    if !operation_category_for_working_unit(record.id, *task_id, ctx)
        .is_some_and(|category| category == OperationCategory::Extraction)
    {
        return None;
    }
    if profile.resolve_clip_name(AnimationClipKey::Work).is_none() {
        return None;
    }
    Some(AnimationClipKey::Work)
}

fn operation_category_for_working_unit(
    unit_id: UnitId,
    task_id: crate::world::TaskId,
    ctx: &WorkPresentationContext<'_>,
) -> Option<OperationCategory> {
    if ctx.world.get_unit(unit_id).is_none() {
        return None;
    }
    let task = ctx.world.task_store().get(task_id)?;
    if task.task_type != TaskType::OperateWorkstation {
        return None;
    }
    let building_id = task.target_building_id();
    let building = ctx.world.get_building(building_id)?;
    let definition = ctx.building_catalog.get(&building.definition_id)?;
    let operation_id = ctx
        .world
        .building_production_store()
        .selected_operation(building_id)
        .cloned()
        .or_else(|| definition.resolved_default_operation())?;
    let operation = ctx.operation_catalog.get(&operation_id)?;
    Some(operation.category)
}
