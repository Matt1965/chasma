use super::id::TaskId;
use crate::world::{BuildingId, UnitId};

/// Structured task/interaction errors (ADR-085 B8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    BuildingNotFound(BuildingId),
    BuildingNotOperational(BuildingId),
    BuildingNotConstructible(BuildingId),
    BuildingCompleted(BuildingId),
    BuildingDestroyed(BuildingId),
    UnitNotEligible(UnitId),
    Unauthorized {
        unit_id: UnitId,
        building_id: BuildingId,
    },
    InteractionPointMissing {
        building_id: BuildingId,
        point_key: String,
    },
    InteractionPointOccupied {
        building_id: BuildingId,
        point_key: String,
    },
    ReservationConflict {
        building_id: BuildingId,
        point_key: String,
    },
    PathUnavailable(UnitId),
    WrongSpace(UnitId),
    OutOfRange(UnitId),
    TaskNotFound(TaskId),
    TaskAlreadyAssigned(TaskId),
    TaskInvalidated(TaskId),
    DefinitionNotFound,
}
