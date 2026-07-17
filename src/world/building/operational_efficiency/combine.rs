use crate::world::building::field_response::{
    EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT, EfficiencyBasisPoints, MAX_EFFICIENCY_BASIS_POINTS,
};

use super::error::OperationalEfficiencyError;

/// Deterministic fixed-point combination of output-efficiency factors (ADR-105 TF5).
///
/// Each factor is in basis points (`10000` = 100%). Factors multiply with rounding:
/// `result = a × b ÷ 10000` (repeated per factor).
pub fn combine_output_efficiency(
    terrain: EfficiencyBasisPoints,
    worker: EfficiencyBasisPoints,
    condition: EfficiencyBasisPoints,
    other: EfficiencyBasisPoints,
) -> Result<EfficiencyBasisPoints, OperationalEfficiencyError> {
    let mut value = terrain.value() as u128;
    for factor in [worker.value(), condition.value(), other.value()] {
        value = value
            .checked_mul(factor as u128)
            .ok_or(OperationalEfficiencyError::EfficiencyMultiplicationOverflow)?;
        value = (value + EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT as u128 / 2)
            / EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT as u128;
    }
    if value > MAX_EFFICIENCY_BASIS_POINTS as u128 {
        return Ok(EfficiencyBasisPoints::new(MAX_EFFICIENCY_BASIS_POINTS));
    }
    Ok(EfficiencyBasisPoints::new(value as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_terrain_differs_in_tf5() {
        let terrain = EfficiencyBasisPoints::new(15_000);
        let hundred = EfficiencyBasisPoints::ONE_HUNDRED_PERCENT;
        let combined = combine_output_efficiency(terrain, hundred, hundred, hundred).unwrap();
        assert_eq!(combined.value(), 15_000);
    }

    #[test]
    fn three_hundred_percent_terrain() {
        let terrain = EfficiencyBasisPoints::new(30_000);
        let hundred = EfficiencyBasisPoints::ONE_HUNDRED_PERCENT;
        let combined = combine_output_efficiency(terrain, hundred, hundred, hundred).unwrap();
        assert_eq!(combined.value(), 30_000);
    }
}
