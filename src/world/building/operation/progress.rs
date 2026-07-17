use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Fixed-point operation progress (`1_000_000` = one completion unit) (ADR-105 TF5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, Default)]
pub struct ProductionProgress(pub u64);

pub const PRODUCTION_PROGRESS_ONE_UNIT: u64 = 1_000_000;

/// Base productive progress contributed per simulation tick before efficiency scaling.
pub const BASE_OPERATION_PROGRESS_PER_TICK: u64 = 10_000;

impl ProductionProgress {
    pub const ZERO: Self = Self(0);

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn add_scaled_base(
        mut self,
        base_progress: u64,
        efficiency_basis_points: u32,
    ) -> Result<Self, super::error::OperationError> {
        let scaled = scale_progress(base_progress, efficiency_basis_points)?;
        self.0 = self
            .0
            .checked_add(scaled)
            .ok_or(super::error::OperationError::OperationProgressOverflow)?;
        Ok(self)
    }

    pub fn completions_since(&mut self, threshold: u64) -> u32 {
        if threshold == 0 {
            return 0;
        }
        let count = (self.0 / threshold) as u32;
        self.0 %= threshold;
        count
    }
}

/// Deterministic scaled progress: `base × efficiency ÷ 10000` with rounding.
pub fn scale_progress(
    base_progress: u64,
    efficiency_basis_points: u32,
) -> Result<u64, super::error::OperationError> {
    let numerator = (base_progress as u128)
        .checked_mul(efficiency_basis_points as u128)
        .ok_or(super::error::OperationError::OperationProgressOverflow)?;
    let scaled = (numerator + 5_000) / 10_000;
    if scaled > u64::MAX as u128 {
        return Err(super::error::OperationError::OperationProgressOverflow);
    }
    Ok(scaled as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;

    #[test]
    fn fifty_percent_produces_half_rate() {
        let half = scale_progress(
            BASE_OPERATION_PROGRESS_PER_TICK,
            EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT / 2,
        )
        .unwrap();
        assert_eq!(half, BASE_OPERATION_PROGRESS_PER_TICK / 2);
    }

    #[test]
    fn one_fifty_percent_produces_one_and_half_rate() {
        let scaled = scale_progress(BASE_OPERATION_PROGRESS_PER_TICK, 15_000).unwrap();
        assert_eq!(scaled, 15_000);
    }

    #[test]
    fn fractional_remainder_retained_across_ticks() {
        let mut progress = ProductionProgress::ZERO;
        for _ in 0..199 {
            progress = progress
                .add_scaled_base(BASE_OPERATION_PROGRESS_PER_TICK, 5_000)
                .unwrap();
        }
        assert_eq!(progress.value(), 995_000);
        let mut copy = progress;
        assert_eq!(copy.completions_since(PRODUCTION_PROGRESS_ONE_UNIT), 0);
        copy = copy
            .add_scaled_base(BASE_OPERATION_PROGRESS_PER_TICK, 5_000)
            .unwrap();
        assert_eq!(copy.completions_since(PRODUCTION_PROGRESS_ONE_UNIT), 1);
        assert_eq!(copy.value(), 0);
    }

    #[test]
    fn three_hundred_percent_triple_rate() {
        let scaled = scale_progress(BASE_OPERATION_PROGRESS_PER_TICK, 30_000).unwrap();
        assert_eq!(scaled, BASE_OPERATION_PROGRESS_PER_TICK * 3);
    }
}
