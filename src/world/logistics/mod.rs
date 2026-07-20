//! Generic hauling and logistics runtime (EP7).

mod register;
pub mod execute;
pub mod generation;
mod id;
mod request;
mod reservation;
mod route;
mod save;
mod step;
mod store;
mod task;
mod types;

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
pub use request::HaulingRequest;
pub use reservation::{
    InventoryReservationSaveState, InventoryReservationStore, available_stack_quantity,
    release_request_reservations, reserve_destination_capacity, reserve_source_items,
};
pub use route::{BuildingLogisticsRouteDefinition, LogisticsEndpointIndex, LogisticsEndpointKey};
pub use register::{
    cancel_logistics_for_building_removal, register_building_logistics_endpoints,
    unregister_building_logistics_endpoints,
};
pub use save::{
    HaulingRequestSaveState, LogisticsSaveState, export_logistics_save_state,
    import_logistics_save_state,
};
pub use step::{HaulTickReport, step_haul_worker_tasks};
pub use store::HaulingRequestStore;
pub use task::{assign_hauling_task, assign_hauling_task_with_priority};
pub use types::{
    HaulExecutionPhase, HaulingBlockingReason, HaulingGenerationReason, HaulingRequestPriority,
    HaulingRequestStatus, HaulingReservationState, LogisticsRouteTrigger,
};
