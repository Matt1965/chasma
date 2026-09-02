//! Truthful execution-path availability for response discovery (Phase 6).

use super::candidate::ResponseBlockingReason;
use super::definition::{ResponseDefinition, ResponseType};

/// Returns `Ok(())` when the response type has a live downstream execution path.
pub fn check_execution_path_available(
    definition: &ResponseDefinition,
) -> Result<(), ResponseBlockingReason> {
    match definition.response_type {
        ResponseType::Trade => Err(ResponseBlockingReason::ExecutionPathUnavailable(
            "trade runtime not implemented".into(),
        )),
        ResponseType::Recruit => Err(ResponseBlockingReason::ExecutionPathUnavailable(
            "recruitment runtime not implemented".into(),
        )),
        ResponseType::RepairBuilding => Err(ResponseBlockingReason::ExecutionPathUnavailable(
            "building repair runtime not implemented".into(),
        )),
        ResponseType::ConstructBuilding => Err(ResponseBlockingReason::ExecutionPathUnavailable(
            "autonomous ConstructBuilding execution not available".into(),
        )),
        ResponseType::IncreaseProduction
        | ResponseType::DecreaseProduction
        | ResponseType::Research
        | ResponseType::Defend
        | ResponseType::Expand => Ok(()),
    }
}
