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
    pub consumption_per_tick: f32,
}

impl NutritionProfile {
    pub fn from_definition(definition: &UnitDefinition) -> Option<Self> {
        if definition.nutrition_consumption_per_tick <= 0.0 {
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
            consumption_per_tick: definition.nutrition_consumption_per_tick,
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

/// Apply one tick of nutrition depletion from the shared authored consumption rate.
pub fn apply_nutrition_decay(state: &mut UnitNutritionState, profile: &NutritionProfile) {
    state.current = (state.current - profile.consumption_per_tick).max(0.0);
}

pub fn restore_nutrition(state: &mut UnitNutritionState, amount: f32, profile: &NutritionProfile) {
    state.current = (state.current + amount).min(profile.max);
}
