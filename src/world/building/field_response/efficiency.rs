use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Terrain output efficiency in basis points (`10000` = 100%, max `30000` = 300%) (ADR-104).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct EfficiencyBasisPoints(pub u32);

pub const EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT: u32 = 10_000;
pub const MAX_EFFICIENCY_BASIS_POINTS: u32 = 30_000;

impl EfficiencyBasisPoints {
    pub const ZERO: Self = Self(0);
    pub const ONE_HUNDRED_PERCENT: Self = Self(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT);

    pub const fn new(value: u32) -> Self {
        Self(if value > MAX_EFFICIENCY_BASIS_POINTS {
            MAX_EFFICIENCY_BASIS_POINTS
        } else {
            value
        })
    }

    pub fn clamped(value: u32) -> Self {
        Self(value.min(MAX_EFFICIENCY_BASIS_POINTS))
    }

    pub fn value(self) -> u32 {
        self.0
    }

    pub fn as_percent_display(self) -> f32 {
        self.0 as f32 / 100.0
    }
}
