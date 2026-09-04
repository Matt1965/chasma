//! Settlement workforce work permissions — autonomous eligibility only (ADR-115 D17).

mod api;
mod domain;
mod mapping;
mod store;

#[cfg(test)]
mod tests;

pub use api::{
    WorkforcePermissionError, clear_settlement_workforce_permissions,
    clear_unit_workforce_permissions, set_unit_work_permission, unit_may_autonomously_perform_work,
    unit_work_allowed,
};
pub use domain::WorkPermissionDomain;
pub use mapping::work_permission_domain_for_task;
pub use store::WorkforcePermissionStore;
