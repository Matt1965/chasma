use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Fixed-point percentage in basis points: `10000` = 100% (ADR-101 TF1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct BasisPoints(pub u16);

pub const BASIS_POINTS_ONE_HUNDRED_PERCENT: u16 = 10_000;

impl BasisPoints {
    pub const ZERO: Self = Self(0);
    pub const ONE_HUNDRED_PERCENT: Self = Self(BASIS_POINTS_ONE_HUNDRED_PERCENT);

    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Coverage ratio in basis points from integer counts.
    pub fn from_ratio(usable: u32, total: u32) -> Result<Self, BasisPointsError> {
        if total == 0 {
            return Err(BasisPointsError::EmptyDenominator);
        }
        let scaled = (usable as u64)
            .checked_mul(BASIS_POINTS_ONE_HUNDRED_PERCENT as u64)
            .ok_or(BasisPointsError::Overflow)?;
        let bp = (scaled / total as u64) as u16;
        Ok(Self(bp))
    }

    pub fn value(self) -> u16 {
        self.0
    }

    pub fn as_percent_display(self) -> f32 {
        self.0 as f32 / 100.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasisPointsError {
    EmptyDenominator,
    Overflow,
}

impl std::fmt::Display for BasisPointsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDenominator => write!(f, "basis points ratio requires non-zero denominator"),
            Self::Overflow => write!(f, "basis points ratio overflow"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_half_is_fifty_percent() {
        assert_eq!(BasisPoints::from_ratio(1, 2).unwrap().0, 5000);
    }

    #[test]
    fn empty_denominator_rejected() {
        assert!(matches!(
            BasisPoints::from_ratio(0, 0),
            Err(BasisPointsError::EmptyDenominator)
        ));
    }
}
