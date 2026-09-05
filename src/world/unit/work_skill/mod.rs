//! Data-driven individual work skills (use-based progression foundation).

mod api;
mod catalog;
mod definition;
mod id;
mod mapping;
mod starter;
mod state;

#[cfg(test)]
mod tests;

pub use api::{
    WorkSkillError, initialize_unit_work_skills, set_work_skill_value, work_skill_value,
};
pub use catalog::{WorkSkillCatalog, WorkSkillCatalogError};
pub use definition::WorkSkillDefinition;
pub use id::WorkSkillId;
pub use mapping::{work_skill_for_permission_domain, work_skill_for_task};
pub use starter::starter_work_skill_definitions;
pub use state::{DEFAULT_WORK_SKILL_VALUE, UnitWorkSkillState};
