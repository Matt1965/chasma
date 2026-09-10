//! Per-unit nutrition / hunger state (ADR-134). Not part of [`super::super::vitals::UnitVitals`].

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::catalog::{
    DEFAULT_HUNGER_CRITICAL_THRESHOLD_FRACTION, DEFAULT_HUNGER_NORMAL_THRESHOLD_FRACTION,
    DEFAULT_NUTRITION_MAX, UnitDefinition,
};

/// Current food fullness for one unit instance. Max/thresholds come from the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Serialize, Deserialize)]
pub struct UnitNutritionState {
    /// Current nutrition amount (0 = depleted, max = well fed).
    pub current: f32,
}

impl Default for UnitNutritionState {
    fn default() -> Self {
        Self { current: 0.0 }
    }
}

impl UnitNutritionState {
    pub fn full(max: f32) -> Self {
        Self {
            current: max.max(0.0),
        }
    }

    pub fn clamped(current: f32, max: f32) -> Self {
        Self {
            current: current.clamp(0.0, max.max(0.0)),
        }
    }
}

/// Hunger urgency stage derived from current nutrition vs authored thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum HungerStage {
    /// Above normal threshold — no self-maintenance pressure.
    Fed,
    /// Between critical and normal — eat when convenient.
    Normal,
    /// At or below critical threshold — may interrupt non-combat work.
    Critical,
}

/// Resolved nutrition profile for one unit definition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NutritionProfile {
    pub max: f32,
    pub normal_threshold: f32,
    pub critical_threshold: f32,
    pub consumption_per_second: f32,
}

impl NutritionProfile {
    pub fn from_definition(definition: &UnitDefinition) -> Option<Self> {
        if definition.nutrition_consumption_per_second <= 0.0 {
            return None;
        }
        let max = if definition.nutrition_max > 0.0 {
            definition.nutrition_max
        } else {
            DEFAULT_NUTRITION_MAX
        };
        let normal_fraction = if definition.hunger_normal_threshold_fraction > 0.0 {
            definition.hunger_normal_threshold_fraction
        } else {
            DEFAULT_HUNGER_NORMAL_THRESHOLD_FRACTION
        };
        let critical_fraction = if definition.hunger_critical_threshold_fraction > 0.0 {
            definition.hunger_critical_threshold_fraction
        } else {
            DEFAULT_HUNGER_CRITICAL_THRESHOLD_FRACTION
        };
        Some(Self {
            max,
            normal_threshold: max * normal_fraction.clamp(0.0, 1.0),
            critical_threshold: max * critical_fraction.clamp(0.0, 1.0),
            consumption_per_second: definition.nutrition_consumption_per_second,
        })
    }
}

pub fn evaluate_hunger_stage(current: f32, profile: &NutritionProfile) -> HungerStage {
    if current <= profile.critical_threshold {
        HungerStage::Critical
    } else if current <= profile.normal_threshold {
        HungerStage::Normal
    } else {
        HungerStage::Fed
    }
}

pub fn hunger_stage_label(stage: HungerStage) -> &'static str {
    match stage {
        HungerStage::Fed => "Fed",
        HungerStage::Normal => "Hungry",
        HungerStage::Critical => "Critical",
    }
}

/// Apply nutrition depletion over `delta_seconds` using the authored per-second rate.
pub fn apply_nutrition_decay(
    state: &mut UnitNutritionState,
    profile: &NutritionProfile,
    delta_seconds: f32,
) {
    let drain = profile.consumption_per_second * delta_seconds.max(0.0);
    state.current = (state.current - drain).max(0.0);
}

pub fn restore_nutrition(state: &mut UnitNutritionState, amount: f32, profile: &NutritionProfile) {
    state.current = (state.current + amount).min(profile.max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::DEFAULT_NUTRITION_CONSUMPTION_PER_SECOND;

    fn sample_profile() -> NutritionProfile {
        NutritionProfile {
            max: 100.0,
            normal_threshold: 50.0,
            critical_threshold: 10.0,
            consumption_per_second: DEFAULT_NUTRITION_CONSUMPTION_PER_SECOND,
        }
    }

    #[test]
    fn baseline_decay_is_point_one_per_real_second() {
        let profile = sample_profile();
        let mut nutrition = UnitNutritionState::full(profile.max);
        apply_nutrition_decay(&mut nutrition, &profile, 1.0);
        assert!((nutrition.current - 99.9).abs() < 1e-4);
    }

    #[test]
    fn decay_is_independent_of_simulation_step_size() {
        let profile = sample_profile();
        let start = 100.0;

        let mut one_step = UnitNutritionState::full(start);
        apply_nutrition_decay(&mut one_step, &profile, 1.0);

        let mut many_steps = UnitNutritionState::full(start);
        for _ in 0..30 {
            apply_nutrition_decay(&mut many_steps, &profile, 1.0 / 30.0);
        }

        assert!((one_step.current - many_steps.current).abs() < 1e-4);
    }

    #[test]
    fn hunger_stages_follow_nutrition_after_decay() {
        let profile = sample_profile();
        let mut nutrition = UnitNutritionState::full(profile.max);
        assert_eq!(
            evaluate_hunger_stage(nutrition.current, &profile),
            HungerStage::Fed
        );

        nutrition.current = profile.normal_threshold;
        assert_eq!(
            evaluate_hunger_stage(nutrition.current, &profile),
            HungerStage::Normal
        );

        nutrition.current = profile.critical_threshold;
        assert_eq!(
            evaluate_hunger_stage(nutrition.current, &profile),
            HungerStage::Critical
        );
    }
}
