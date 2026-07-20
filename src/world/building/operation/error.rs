use crate::world::BuildingId;
use crate::world::UnitId;
use crate::world::building::field_response::EfficiencyBasisPoints;
use crate::world::building::operational_efficiency::OperationalLimitingFactor;

/// Operation stepping failures (ADR-105 TF5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationError {
    BuildingNotFound(crate::world::BuildingId),
    WorkerNotFound(UnitId),
    ReservationInvalid,
    OperationStateMissing(crate::world::BuildingId),
    OperationProgressOverflow,
    OutputDestinationMissing,
    OutputDestinationFull,
    OutputCreationFailed,
    OperationBlocked(OperationalLimitingFactor),
    StaleOperationRevision,
}

impl std::fmt::Display for OperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildingNotFound(id) => write!(f, "building `{id:?}` not found"),
            Self::WorkerNotFound(id) => write!(f, "worker `{id:?}` not found"),
            Self::ReservationInvalid => write!(f, "task reservation invalid"),
            Self::OperationStateMissing(id) => write!(f, "operation state missing for `{id:?}`"),
            Self::OperationProgressOverflow => write!(f, "operation progress overflow"),
            Self::OutputDestinationMissing => write!(f, "output destination missing"),
            Self::OutputDestinationFull => write!(f, "output destination full"),
            Self::OutputCreationFailed => write!(f, "output creation failed"),
            Self::OperationBlocked(factor) => write!(f, "operation blocked: {}", factor.label()),
            Self::StaleOperationRevision => write!(f, "stale operation revision"),
        }
    }
}

/// Per-tick workstation operation report (ADR-105 TF5).
#[derive(Debug, Clone, PartialEq)]
pub struct OperationStepReport {
    pub building_id: crate::world::BuildingId,
    pub worker_id: UnitId,
    pub base_progress: u64,
    pub terrain_efficiency_bp: u32,
    pub final_efficiency_bp: u32,
    pub scaled_progress: u64,
    pub accumulated_progress: u64,
    pub completions: u32,
    pub can_operate: bool,
    pub limiting_factor: OperationalLimitingFactor,
    pub lifecycle: super::lifecycle::OperationLifecycle,
    pub selected_operation: Option<super::operation_id::OperationDefinitionId>,
}

/// Completion summary for one operation threshold crossing (ADR-105 TF5).
#[derive(Debug, Clone, PartialEq)]
pub struct OperationCompletionReport {
    pub building_id: crate::world::BuildingId,
    pub completed_units: u32,
    pub leftover_progress: u64,
    pub blocked: bool,
    pub blocked_reason: Option<OperationalLimitingFactor>,
}
