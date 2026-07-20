//! Marketplace listings discovered from TaskStore + open hauling requests (SA7).

use crate::world::logistics::HaulingRequestId;
use crate::world::task::{TaskId, TaskPriority, TaskType};
use crate::world::BuildingId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceListingKind {
    Task,
    HaulRequest,
}

/// One piece of work visible in the shared marketplace.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketplaceListing {
    pub kind: MarketplaceListingKind,
    pub task_id: Option<TaskId>,
    pub haul_request_id: Option<HaulingRequestId>,
    pub building_id: BuildingId,
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub created_tick: u64,
}

/// Scored candidate for one worker evaluation (diagnostics).
#[derive(Debug, Clone, PartialEq)]
pub struct MarketplaceCandidate {
    pub listing: MarketplaceListing,
    pub distance_meters: f32,
    pub score: f32,
    pub eligible: bool,
    pub block_reason: Option<String>,
}
