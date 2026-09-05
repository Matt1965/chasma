//! Authoritative read/write API for per-unit work skills.

use crate::world::{UnitId, WorldData};

use super::catalog::WorkSkillCatalog;
use super::id::WorkSkillId;
use super::state::UnitWorkSkillState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkSkillError {
    UnitNotFound,
    UnknownWorkSkill,
}

/// Initialize work skill state for a newly created unit.
pub fn initialize_unit_work_skills(work_skills: &mut UnitWorkSkillState) {
    work_skills.overrides_mut().clear();
}

/// Current value for one authored work skill on an individual unit.
pub fn work_skill_value(
    world: &WorldData,
    catalog: &WorkSkillCatalog,
    unit_id: UnitId,
    skill_id: &WorkSkillId,
) -> Result<i64, WorkSkillError> {
    if catalog.get(skill_id).is_none() {
        return Err(WorkSkillError::UnknownWorkSkill);
    }
    let record = world
        .get_unit(unit_id)
        .ok_or(WorkSkillError::UnitNotFound)?;
    Ok(record.work_skills.resolve(skill_id))
}

/// Set one work skill value on an individual unit.
pub fn set_work_skill_value(
    world: &mut WorldData,
    catalog: &WorkSkillCatalog,
    unit_id: UnitId,
    skill_id: &WorkSkillId,
    value: i64,
) -> Result<(), WorkSkillError> {
    if catalog.get(skill_id).is_none() {
        return Err(WorkSkillError::UnknownWorkSkill);
    }
    world
        .mutate_unit(unit_id, |record| {
            record.work_skills.set(skill_id.clone(), value)
        })
        .ok_or(WorkSkillError::UnitNotFound)?;
    Ok(())
}
