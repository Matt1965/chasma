use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::efficiency::{EfficiencyBasisPoints, MAX_EFFICIENCY_BASIS_POINTS};
use super::error::FieldResponseProfileError;
use super::id::{FieldResponseProfileId, validate_field_response_profile_id};

/// One knot on a piecewise-linear response curve.
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct FieldResponsePoint {
    pub field_value: u16,
    pub efficiency_basis_points: u32,
}

/// Authoritative reusable terrain-field response curve (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct FieldResponseProfileDefinition {
    pub id: FieldResponseProfileId,
    pub display_name: String,
    pub description: String,
    pub points: Vec<FieldResponsePoint>,
    pub enabled: bool,
    /// Optional stricter ceiling than the global 300% safety maximum.
    pub max_efficiency_override: Option<u32>,
}

impl FieldResponseProfileDefinition {
    pub fn new(id: impl Into<FieldResponseProfileId>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            description: String::new(),
            points: Vec::new(),
            enabled: true,
            max_efficiency_override: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_points(mut self, points: Vec<FieldResponsePoint>) -> Self {
        self.points = points;
        self
    }

    pub fn effective_max_efficiency(&self) -> u32 {
        self.max_efficiency_override
            .unwrap_or(MAX_EFFICIENCY_BASIS_POINTS)
            .min(MAX_EFFICIENCY_BASIS_POINTS)
    }

    /// Validate profile metadata and points before catalog insertion.
    pub fn validate(&self) -> Result<(), FieldResponseProfileError> {
        validate_field_response_profile_id(self.id.as_str())
            .map_err(FieldResponseProfileError::InvalidProfileId)?;
        if self.points.len() < 2 {
            return Err(FieldResponseProfileError::TooFewPoints(self.id.clone()));
        }
        let max_eff = self.effective_max_efficiency();
        let mut prev_value: Option<u16> = None;
        for point in &self.points {
            if let Some(prev) = prev_value {
                if point.field_value <= prev {
                    return Err(FieldResponseProfileError::PointsUnsorted(self.id.clone()));
                }
            }
            prev_value = Some(point.field_value);
            if point.efficiency_basis_points > max_eff {
                return Err(FieldResponseProfileError::EfficiencyOutOfRange {
                    profile_id: self.id.clone(),
                    efficiency_basis_points: point.efficiency_basis_points,
                });
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for point in &self.points {
            if !seen.insert(point.field_value) {
                return Err(FieldResponseProfileError::DuplicatePoint {
                    profile_id: self.id.clone(),
                    field_value: point.field_value,
                });
            }
        }
        Ok(())
    }

    /// Build a validated profile with sorted unique points.
    pub fn from_points(
        id: FieldResponseProfileId,
        display_name: impl Into<String>,
        mut points: Vec<FieldResponsePoint>,
    ) -> Result<Self, FieldResponseProfileError> {
        points.sort_by_key(|point| point.field_value);
        let profile = Self::new(id, display_name).with_points(points);
        profile.validate()?;
        Ok(profile)
    }
}

/// Convert a display percent (0–300) into efficiency basis points.
pub fn efficiency_basis_points_from_percent(percent: f32) -> u32 {
    let bp = (percent * 100.0).round() as i64;
    bp.clamp(0, MAX_EFFICIENCY_BASIS_POINTS as i64) as u32
}

/// Convert a field display percent (0–100) into raw `u16` field storage.
pub fn field_value_from_percent(percent: f32) -> u16 {
    let scaled = (percent / 100.0 * 65535.0).round() as i64;
    scaled.clamp(0, u16::MAX as i64) as u16
}

/// Display percent for a raw field `u16` value.
pub fn field_value_to_percent_display(value: u16) -> f32 {
    value as f32 / 65535.0 * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_percent_round_trip_is_stable() {
        let value = field_value_from_percent(50.0);
        assert!((field_value_to_percent_display(value) - 50.0).abs() < 0.2);
    }
}
