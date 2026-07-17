//! Baseline scale calculation from desired dimensions and source bounds (ADR-097 DT1).

use bevy::prelude::*;

use crate::world::authoring_transform::{AuthoringScale, FixedScale};

use super::definition::{SizeReferenceAxis, SourceDimensions};
use super::error::AssetSizingError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizingPolicy {
    /// Doodad: all three desired dimensions → non-uniform XYZ baseline.
    DoodadNonUniform,
    /// Doodad/Unit/Building: one reference axis → uniform baseline.
    ReferenceAxisUniform,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaselineScaleResult {
    pub baseline_scale: AuthoringScale,
    pub approximate_final_dimensions: SourceDimensions,
    pub per_axis_deviation: Vec3,
    pub exact_scale_f32: Vec3,
}

pub fn calculate_baseline_scale(
    policy: SizingPolicy,
    source: SourceDimensions,
    desired_width: Option<f32>,
    desired_height: Option<f32>,
    desired_depth: Option<f32>,
    reference_axis: Option<SizeReferenceAxis>,
) -> Result<BaselineScaleResult, AssetSizingError> {
    if !source.is_valid() {
        return Err(AssetSizingError::SourceBoundsInvalid {
            message: "non-finite or non-positive source extents".into(),
        });
    }

    let desired_count = [desired_width, desired_height, desired_depth]
        .into_iter()
        .filter(|v| v.is_some())
        .count();

    if desired_count == 0 {
        return Err(AssetSizingError::DesiredDimensionsInvalid {
            message: "no desired dimensions supplied".into(),
        });
    }

    match policy {
        SizingPolicy::DoodadNonUniform if desired_count == 3 => {
            let scale_x = ratio(desired_width.unwrap(), source.width_meters, "width")?;
            let scale_y = ratio(desired_height.unwrap(), source.height_meters, "height")?;
            let scale_z = ratio(desired_depth.unwrap(), source.depth_meters, "depth")?;
            finalize(
                AuthoringScale::from_non_uniform_f32(scale_x, scale_y, scale_z)
                    .map_err(|_| AssetSizingError::BaselineScaleOutOfRange)?,
                source,
                Vec3::new(scale_x, scale_y, scale_z),
                Vec3::ZERO,
            )
        }
        SizingPolicy::DoodadNonUniform | SizingPolicy::ReferenceAxisUniform => {
            if desired_count == 3 && matches!(policy, SizingPolicy::DoodadNonUniform) {
                unreachable!()
            }
            let axis = reference_axis.ok_or(AssetSizingError::InvalidReferenceAxis)?;
            let desired = desired_for_axis(axis, desired_width, desired_height, desired_depth)
                .ok_or(AssetSizingError::AmbiguousPartialDimensions)?;
            let source_axis = source.axis(axis);
            let uniform = ratio(desired, source_axis, axis.label())?;
            let baseline = AuthoringScale::from_uniform_f32(uniform)
                .map_err(|_| AssetSizingError::BaselineScaleOutOfRange)?;
            let scale_vec = Vec3::splat(uniform);
            let final_dims = SourceDimensions {
                width_meters: source.width_meters * uniform,
                height_meters: source.height_meters * uniform,
                depth_meters: source.depth_meters * uniform,
            };
            let deviation = Vec3::new(
                deviation(desired_width, final_dims.width_meters),
                deviation(desired_height, final_dims.height_meters),
                deviation(desired_depth, final_dims.depth_meters),
            );
            finalize(baseline, source, scale_vec, deviation)
        }
    }
}

fn ratio(desired: f32, source: f32, axis: &str) -> Result<f32, AssetSizingError> {
    if !desired.is_finite() || desired <= 0.0 {
        return Err(AssetSizingError::DesiredDimensionsInvalid {
            message: format!("invalid desired {axis}"),
        });
    }
    if source <= 0.0 || !source.is_finite() {
        return Err(AssetSizingError::SourceAxisZero {
            axis: axis.to_string(),
        });
    }
    Ok(desired / source)
}

fn desired_for_axis(
    axis: SizeReferenceAxis,
    width: Option<f32>,
    height: Option<f32>,
    depth: Option<f32>,
) -> Option<f32> {
    match axis {
        SizeReferenceAxis::Width => width,
        SizeReferenceAxis::Height => height,
        SizeReferenceAxis::Depth => depth,
    }
}

fn deviation(desired: Option<f32>, actual: f32) -> f32 {
    desired.map(|d| actual - d).unwrap_or(0.0)
}

fn finalize(
    baseline: AuthoringScale,
    source: SourceDimensions,
    exact_scale_f32: Vec3,
    per_axis_deviation: Vec3,
) -> Result<BaselineScaleResult, AssetSizingError> {
    let scale_vec = baseline.to_vec3();
    Ok(BaselineScaleResult {
        approximate_final_dimensions: SourceDimensions {
            width_meters: source.width_meters * scale_vec.x,
            height_meters: source.height_meters * scale_vec.y,
            depth_meters: source.depth_meters * scale_vec.z,
        },
        per_axis_deviation,
        exact_scale_f32,
        baseline_scale: baseline,
    })
}

pub fn quantize_baseline_scale(scale: AuthoringScale) -> Result<AuthoringScale, AssetSizingError> {
    match scale {
        AuthoringScale::Uniform(value) => {
            let quantized = FixedScale::from_milli(value.milli())
                .map_err(|_| AssetSizingError::BaselineScaleOutOfRange)?;
            Ok(AuthoringScale::Uniform(quantized))
        }
        AuthoringScale::NonUniform { x, y, z } => {
            let x = FixedScale::from_milli(x.milli())
                .map_err(|_| AssetSizingError::BaselineScaleOutOfRange)?;
            let y = FixedScale::from_milli(y.milli())
                .map_err(|_| AssetSizingError::BaselineScaleOutOfRange)?;
            let z = FixedScale::from_milli(z.milli())
                .map_err(|_| AssetSizingError::BaselineScaleOutOfRange)?;
            Ok(AuthoringScale::NonUniform { x, y, z })
        }
    }
}

pub fn check_suspected_unit_mismatch(
    source: SourceDimensions,
    desired: Option<f32>,
) -> Option<String> {
    let desired = desired?;
    let max_source = source
        .width_meters
        .max(source.height_meters)
        .max(source.depth_meters);
    let min_source = source
        .width_meters
        .min(source.height_meters)
        .min(source.depth_meters);
    let ratio = desired / max_source;
    if max_source > desired * 50.0 || min_source < desired / 50.0 || ratio > 50.0 || ratio < 0.02 {
        Some(format!(
            "source bounds ({max_source:.3} m max) vs desired ({desired:.3} m) — check cm/mm export"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(w: f32, h: f32, d: f32) -> SourceDimensions {
        SourceDimensions {
            width_meters: w,
            height_meters: h,
            depth_meters: d,
        }
    }

    #[test]
    fn doodad_full_xyz_non_uniform() {
        let result = calculate_baseline_scale(
            SizingPolicy::DoodadNonUniform,
            source(2.0, 1.0, 4.0),
            Some(1.0),
            Some(2.0),
            Some(2.0),
            None,
        )
        .unwrap();
        assert_eq!(result.exact_scale_f32, Vec3::new(0.5, 2.0, 0.5));
    }

    #[test]
    fn unit_height_reference_uniform() {
        let result = calculate_baseline_scale(
            SizingPolicy::ReferenceAxisUniform,
            source(0.81, 0.81, 0.4),
            None,
            Some(1.75),
            None,
            Some(SizeReferenceAxis::Height),
        )
        .unwrap();
        let uniform = result.exact_scale_f32.x;
        assert!((uniform - 1.75 / 0.81).abs() < 0.01);
    }

    #[test]
    fn ambiguous_partial_rejected() {
        let err = calculate_baseline_scale(
            SizingPolicy::ReferenceAxisUniform,
            source(1.0, 1.0, 1.0),
            Some(2.0),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, AssetSizingError::InvalidReferenceAxis));
    }

    #[test]
    fn zero_source_rejected() {
        let err = calculate_baseline_scale(
            SizingPolicy::ReferenceAxisUniform,
            source(1.0, 0.0, 1.0),
            None,
            Some(1.0),
            None,
            Some(SizeReferenceAxis::Height),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AssetSizingError::SourceBoundsInvalid { .. } | AssetSizingError::SourceAxisZero { .. }
        ));
    }
}
