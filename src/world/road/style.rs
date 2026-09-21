use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::error::RoadError;

/// Authored road style category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, PartialOrd, Ord)]
pub enum RoadStyleId {
    Trail,
    DirtRoad,
    MajorRoad,
}

impl RoadStyleId {
    pub fn default_for_v1() -> Self {
        Self::DirtRoad
    }
}

/// Default presentation and future gameplay parameters for a road style.
///
/// Terrain and pathfinding effects are not applied in Phase 1; fields exist so
/// authored roads can carry stable metadata before those systems exist.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct RoadStyleDefaults {
    pub width_m: f32,
    pub shoulder_m: f32,
    pub darkening_strength: f32,
    pub depression_m: f32,
    pub flatten_strength: f32,
    pub longitudinal_smooth_m: f32,
    /// Reserved for future hierarchical pathfinding cost multipliers.
    pub movement_cost_multiplier: f32,
}

impl RoadStyleDefaults {
    pub fn trail() -> Self {
        Self {
            width_m: 2.5,
            shoulder_m: 1.0,
            darkening_strength: 0.25,
            depression_m: 0.04,
            flatten_strength: 0.45,
            longitudinal_smooth_m: 6.0,
            movement_cost_multiplier: 0.85,
        }
    }

    pub fn dirt_road() -> Self {
        Self {
            width_m: 4.0,
            shoulder_m: 2.0,
            darkening_strength: 0.35,
            depression_m: 0.08,
            flatten_strength: 0.65,
            longitudinal_smooth_m: 10.0,
            movement_cost_multiplier: 0.7,
        }
    }

    pub fn major_road() -> Self {
        Self {
            width_m: 6.0,
            shoulder_m: 3.0,
            darkening_strength: 0.45,
            depression_m: 0.12,
            flatten_strength: 0.8,
            longitudinal_smooth_m: 14.0,
            movement_cost_multiplier: 0.55,
        }
    }

    pub fn validate(&self) -> Result<(), RoadError> {
        if !self.width_m.is_finite() || self.width_m <= 0.0 {
            return Err(RoadError::InvalidStyleValue(
                "width_m must be finite and positive".to_string(),
            ));
        }
        if !self.shoulder_m.is_finite() || self.shoulder_m < 0.0 {
            return Err(RoadError::InvalidStyleValue(
                "shoulder_m must be finite and non-negative".to_string(),
            ));
        }
        for (name, value) in [
            ("darkening_strength", self.darkening_strength),
            ("flatten_strength", self.flatten_strength),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(RoadError::InvalidStyleValue(format!(
                    "{name} must be finite and within [0, 1]"
                )));
            }
        }
        if !self.depression_m.is_finite() || self.depression_m < 0.0 {
            return Err(RoadError::InvalidStyleValue(
                "depression_m must be finite and non-negative".to_string(),
            ));
        }
        if !self.longitudinal_smooth_m.is_finite() || self.longitudinal_smooth_m <= 0.0 {
            return Err(RoadError::InvalidStyleValue(
                "longitudinal_smooth_m must be finite and positive".to_string(),
            ));
        }
        if !self.movement_cost_multiplier.is_finite() || self.movement_cost_multiplier <= 0.0 {
            return Err(RoadError::InvalidStyleValue(
                "movement_cost_multiplier must be finite and positive".to_string(),
            ));
        }
        Ok(())
    }
}

/// Optional per-road overrides for a subset of style fields.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize, Default)]
pub struct RoadStyleOverrides {
    pub width_m: Option<f32>,
    pub shoulder_m: Option<f32>,
    pub darkening_strength: Option<f32>,
    pub depression_m: Option<f32>,
    pub flatten_strength: Option<f32>,
    pub longitudinal_smooth_m: Option<f32>,
    pub movement_cost_multiplier: Option<f32>,
}

impl RoadStyleOverrides {
    pub fn validate(&self) -> Result<(), RoadError> {
        if let Some(width_m) = self.width_m {
            if !width_m.is_finite() || width_m <= 0.0 {
                return Err(RoadError::InvalidStyleValue(
                    "override width_m must be finite and positive".to_string(),
                ));
            }
        }
        if let Some(shoulder_m) = self.shoulder_m {
            if !shoulder_m.is_finite() || shoulder_m < 0.0 {
                return Err(RoadError::InvalidStyleValue(
                    "override shoulder_m must be finite and non-negative".to_string(),
                ));
            }
        }
        for (name, value) in [
            ("darkening_strength", self.darkening_strength),
            ("flatten_strength", self.flatten_strength),
        ] {
            if let Some(value) = value {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(RoadError::InvalidStyleValue(format!(
                        "override {name} must be finite and within [0, 1]"
                    )));
                }
            }
        }
        if let Some(depression_m) = self.depression_m {
            if !depression_m.is_finite() || depression_m < 0.0 {
                return Err(RoadError::InvalidStyleValue(
                    "override depression_m must be finite and non-negative".to_string(),
                ));
            }
        }
        if let Some(longitudinal_smooth_m) = self.longitudinal_smooth_m {
            if !longitudinal_smooth_m.is_finite() || longitudinal_smooth_m <= 0.0 {
                return Err(RoadError::InvalidStyleValue(
                    "override longitudinal_smooth_m must be finite and positive".to_string(),
                ));
            }
        }
        if let Some(movement_cost_multiplier) = self.movement_cost_multiplier {
            if !movement_cost_multiplier.is_finite() || movement_cost_multiplier <= 0.0 {
                return Err(RoadError::InvalidStyleValue(
                    "override movement_cost_multiplier must be finite and positive".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub fn resolve(&self, defaults: &RoadStyleDefaults) -> RoadStyleDefaults {
        RoadStyleDefaults {
            width_m: self.width_m.unwrap_or(defaults.width_m),
            shoulder_m: self.shoulder_m.unwrap_or(defaults.shoulder_m),
            darkening_strength: self
                .darkening_strength
                .unwrap_or(defaults.darkening_strength),
            depression_m: self.depression_m.unwrap_or(defaults.depression_m),
            flatten_strength: self.flatten_strength.unwrap_or(defaults.flatten_strength),
            longitudinal_smooth_m: self
                .longitudinal_smooth_m
                .unwrap_or(defaults.longitudinal_smooth_m),
            movement_cost_multiplier: self
                .movement_cost_multiplier
                .unwrap_or(defaults.movement_cost_multiplier),
        }
    }
}

pub fn default_style_table() -> std::collections::BTreeMap<RoadStyleId, RoadStyleDefaults> {
    std::collections::BTreeMap::from([
        (RoadStyleId::Trail, RoadStyleDefaults::trail()),
        (RoadStyleId::DirtRoad, RoadStyleDefaults::dirt_road()),
        (RoadStyleId::MajorRoad, RoadStyleDefaults::major_road()),
    ])
}
