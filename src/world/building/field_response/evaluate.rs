//! Piecewise-linear field response evaluation (ADR-104 TF4).
//!
//! Endpoint behavior:
//! - Below the first point: clamp to first point efficiency.
//! - Above the last point: clamp to last point efficiency.
//! - Between points: linear interpolation with integer rounding.

use super::definition::FieldResponseProfileDefinition;
use super::efficiency::EfficiencyBasisPoints;
use super::error::FieldResponseEvaluationError;

/// Authoritative response evaluation for one field sample value.
pub fn evaluate_field_response(
    profile: &FieldResponseProfileDefinition,
    field_value: u16,
) -> Result<EfficiencyBasisPoints, FieldResponseEvaluationError> {
    if !profile.enabled {
        return Err(FieldResponseEvaluationError::ProfileDisabled(
            profile.id.clone(),
        ));
    }
    if profile.points.is_empty() {
        return Err(FieldResponseEvaluationError::PointsEmpty(
            profile.id.clone(),
        ));
    }
    if profile.points.len() < 2 {
        return Err(FieldResponseEvaluationError::MalformedProfile(
            profile.id.clone(),
        ));
    }

    let max_eff = profile.effective_max_efficiency();
    let first = &profile.points[0];
    let last = profile.points.last().expect("validated profile has points");

    let raw = if field_value <= first.field_value {
        first.efficiency_basis_points
    } else if field_value >= last.field_value {
        last.efficiency_basis_points
    } else {
        let (left, right) = find_segment(&profile.points, field_value);
        interpolate_efficiency(left, right, field_value)
    };

    Ok(EfficiencyBasisPoints::new(raw.min(max_eff)))
}

fn find_segment(
    points: &[super::definition::FieldResponsePoint],
    value: u16,
) -> (
    &super::definition::FieldResponsePoint,
    &super::definition::FieldResponsePoint,
) {
    for window in points.windows(2) {
        let left = &window[0];
        let right = &window[1];
        if value >= left.field_value && value <= right.field_value {
            return (left, right);
        }
    }
    let len = points.len();
    (&points[len - 2], &points[len - 1])
}

fn interpolate_efficiency(
    left: &super::definition::FieldResponsePoint,
    right: &super::definition::FieldResponsePoint,
    field_value: u16,
) -> u32 {
    if left.field_value == right.field_value {
        return left.efficiency_basis_points;
    }
    let value_delta = (field_value - left.field_value) as u64;
    let value_span = (right.field_value - left.field_value) as u64;
    let eff_delta = right.efficiency_basis_points as i64 - left.efficiency_basis_points as i64;
    let numerator = eff_delta * value_delta as i64 + (value_span as i64 / 2);
    let interpolated = left.efficiency_basis_points as i64 + numerator / value_span as i64;
    interpolated.clamp(0, super::efficiency::MAX_EFFICIENCY_BASIS_POINTS as i64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::field_response::definition::{
        FieldResponsePoint, field_value_from_percent,
    };
    use crate::world::building::field_response::efficiency::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
    use crate::world::building::field_response::id::FieldResponseProfileId;
    use crate::world::building::field_response::starter;

    fn eval_percent(profile_id: &str, field_percent: f32) -> u32 {
        let profile = starter::starter_profiles()
            .into_iter()
            .find(|profile| profile.id.as_str() == profile_id)
            .expect("profile");
        evaluate_field_response(&profile, field_value_from_percent(field_percent))
            .unwrap()
            .value()
    }

    #[test]
    fn iron_mine_monotonic_curve() {
        assert_eq!(eval_percent("iron_mine_monotonic", 0.0), 0);
        assert_eq!(eval_percent("iron_mine_monotonic", 20.0), 0);
        assert_eq!(eval_percent("iron_mine_monotonic", 50.0), 5_000);
        assert_eq!(eval_percent("iron_mine_monotonic", 80.0), 10_000);
        assert_eq!(eval_percent("iron_mine_monotonic", 100.0), 12_000);
    }

    #[test]
    fn water_crop_preferred_range() {
        assert_eq!(eval_percent("water_crop_preferred_range", 0.0), 0);
        assert_eq!(eval_percent("water_crop_preferred_range", 20.0), 4_000);
        assert_eq!(eval_percent("water_crop_preferred_range", 45.0), 10_000);
        assert_eq!(eval_percent("water_crop_preferred_range", 70.0), 10_000);
        let ninety = eval_percent("water_crop_preferred_range", 90.0);
        assert!((2_900..=3_400).contains(&ninety), "90% was {ninety}");
        assert_eq!(eval_percent("water_crop_preferred_range", 100.0), 0);
    }

    #[test]
    fn exact_point_hit() {
        let profile = FieldResponseProfileDefinition::from_points(
            FieldResponseProfileId::new("test"),
            "Test",
            vec![
                FieldResponsePoint {
                    field_value: 0,
                    efficiency_basis_points: 0,
                },
                FieldResponsePoint {
                    field_value: 10_000,
                    efficiency_basis_points: EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            evaluate_field_response(&profile, 10_000).unwrap().value(),
            EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT
        );
    }

    #[test]
    fn below_first_clamps() {
        let profile = FieldResponseProfileDefinition::from_points(
            FieldResponseProfileId::new("test"),
            "Test",
            vec![
                FieldResponsePoint {
                    field_value: 1_000,
                    efficiency_basis_points: 2_000,
                },
                FieldResponsePoint {
                    field_value: 5_000,
                    efficiency_basis_points: 8_000,
                },
            ],
        )
        .unwrap();
        assert_eq!(evaluate_field_response(&profile, 0).unwrap().value(), 2_000);
    }

    #[test]
    fn above_last_clamps() {
        let profile = FieldResponseProfileDefinition::from_points(
            FieldResponseProfileId::new("test"),
            "Test",
            vec![
                FieldResponsePoint {
                    field_value: 1_000,
                    efficiency_basis_points: 2_000,
                },
                FieldResponsePoint {
                    field_value: 5_000,
                    efficiency_basis_points: 8_000,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            evaluate_field_response(&profile, u16::MAX).unwrap().value(),
            8_000
        );
    }

    #[test]
    fn over_one_hundred_percent_supported() {
        let profile = FieldResponseProfileDefinition::from_points(
            FieldResponseProfileId::new("rich"),
            "Rich",
            vec![
                FieldResponsePoint {
                    field_value: 0,
                    efficiency_basis_points: 0,
                },
                FieldResponsePoint {
                    field_value: u16::MAX,
                    efficiency_basis_points: 25_000,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            evaluate_field_response(&profile, u16::MAX).unwrap().value(),
            25_000
        );
    }

    #[test]
    fn three_hundred_percent_clamp() {
        let profile = FieldResponseProfileDefinition::from_points(
            FieldResponseProfileId::new("max"),
            "Max",
            vec![
                FieldResponsePoint {
                    field_value: 0,
                    efficiency_basis_points: 0,
                },
                FieldResponsePoint {
                    field_value: u16::MAX,
                    efficiency_basis_points: 30_000,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            evaluate_field_response(&profile, u16::MAX).unwrap().value(),
            super::super::efficiency::MAX_EFFICIENCY_BASIS_POINTS
        );
    }

    #[test]
    fn disabled_profile_rejected() {
        let mut profile = FieldResponseProfileDefinition::from_points(
            FieldResponseProfileId::new("disabled"),
            "Disabled",
            vec![
                FieldResponsePoint {
                    field_value: 0,
                    efficiency_basis_points: 0,
                },
                FieldResponsePoint {
                    field_value: 100,
                    efficiency_basis_points: 100,
                },
            ],
        )
        .unwrap();
        profile.enabled = false;
        assert!(matches!(
            evaluate_field_response(&profile, 50),
            Err(FieldResponseEvaluationError::ProfileDisabled(_))
        ));
    }
}
