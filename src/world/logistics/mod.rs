//! Generic hauling and logistics runtime (EP7).

pub mod execute;
pub mod generation;
mod id;
pub mod misfiled;
mod recovery;
mod register;
mod request;
mod reservation;
mod route;
mod save;
mod step;
mod store;
mod task;
mod types;

#[cfg(test)]
mod cargo_tests;
#[cfg(test)]
mod tests;

pub use execute::{
    cancel_hauling_request, deposit_haul_cargo, force_complete_hauling_request, pickup_haul_cargo,
    reserve_hauling_request,
};
pub use generation::{
    spawn_manual_hauling_request, sync_logistics_requests_from_assessment,
    sync_output_surplus_after_production,
};
pub use id::HaulingRequestId;
pub use misfiled::{
    sync_dirty_storage_logistics, sync_misfiled_storage_for_building,
};
pub use recovery::retry_blocked_hauling_requests;
pub use register::{
    cancel_logistics_for_building_removal, register_building_logistics_endpoints,
    unregister_building_logistics_endpoints,
};
pub use request::HaulingRequest;
pub use reservation::{
    InventoryReservationStore, available_stack_quantity,
    destination_can_fit_stack_quantity,
};
pub use route::{BuildingLogisticsRouteDefinition, LogisticsEndpointIndex};
pub use save::{
    LogisticsSaveState, export_logistics_save_state,
    import_logistics_save_state,
};
pub use step::{HaulTickReport, step_haul_worker_tasks};
pub use store::HaulingRequestStore;
pub use task::{assign_hauling_task, assign_hauling_task_with_priority};
pub use types::{
    HaulingRequestPriority,
    HaulingRequestStatus, LogisticsRouteTrigger,
};
