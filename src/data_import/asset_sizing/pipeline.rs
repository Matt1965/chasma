//! Asset sizing resolution during offline import (ADR-097 DT1).

use std::path::Path;

use crate::world::asset_sizing::{
    AssetSizingDefinition, AssetSizingError, AssetSizingReport, BaselineScaleResult,
    SizeReferenceAxis, SizingMigrationState, SizingPolicy, SourceBoundsOrigin, SourceDimensions,
    calculate_baseline_scale, check_suspected_unit_mismatch,
    normalize_source_dimensions_to_desired, quantize_baseline_scale,
};
use crate::world::authoring_transform::{AuthoringScale, BuildingTransformSafetyClass};

use super::bounds::{asset_path_for_render_key, measure_glb_source_bounds};
use super::targets::apply_building_footprint_sizing_targets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSizingKind {
    Unit,
    Doodad,
    Building {
        safety_class: BuildingTransformSafetyClass,
    },
}

pub struct SizingResolveInput<'a> {
    pub definition_id: &'a str,
    pub render_key: &'a str,
    pub asset_root: &'a str,
    pub kind: ContentSizingKind,
    pub sizing: &'a mut AssetSizingDefinition,
    pub legacy_uniform_scale: Option<f32>,
    pub building_footprint_width_meters: Option<f32>,
    pub building_footprint_depth_meters: Option<f32>,
    /// When Desired*M columns are absent, doodad import may infer height from kind defaults.
    pub doodad_kind_height_hint_meters: Option<f32>,
    /// When Desired Height M is absent, unit import may infer from id / collision radius.
    pub unit_height_hint_meters: Option<f32>,
}

pub fn resolve_content_sizing(mut input: SizingResolveInput<'_>) -> AssetSizingReport {
    let mut report = AssetSizingReport {
        definition_id: input.definition_id.to_string(),
        definition_kind: kind_label(input.kind).to_string(),
        asset_path: asset_path_for_render_key(input.asset_root, input.render_key)
            .display()
            .to_string(),
        render_key: input.render_key.to_string(),
        desired_width_meters: input.sizing.desired_width_meters,
        desired_height_meters: input.sizing.desired_height_meters,
        desired_depth_meters: input.sizing.desired_depth_meters,
        reference_axis: input.sizing.size_reference_axis,
        rotation_correction: input.sizing.rotation_correction,
        model_offset: input.sizing.model_local_offset_meters.to_array(),
        ..Default::default()
    };

    if input.sizing.has_explicit_baseline() && input.sizing.has_desired_dimensions() {
        report.errors.push(
            AssetSizingError::ContradictorySizingInputs {
                message: "explicit baseline scale and desired dimensions both supplied".into(),
            }
            .to_string(),
        );
        apply_legacy_fallback(input.sizing, input.legacy_uniform_scale);
        return report;
    }

    if let Some(explicit) = input.sizing.explicit_baseline_scale {
        match quantize_baseline_scale(explicit) {
            Ok(scale) => {
                input.sizing.calculated_baseline_scale = Some(scale);
                input.sizing.migration_state = SizingMigrationState::MetricConfigured;
                report.quantized_baseline_scale = Some(scale);
                report.exact_calculated_scale = Some(scale.to_vec3().to_array());
            }
            Err(err) => report.errors.push(err.to_string()),
        }
        return report;
    }

    if input.sizing.has_desired_dimensions() {
        match resolve_from_desired_dimensions(&mut input) {
            Ok((result, warnings)) => {
                apply_baseline_result(input.sizing, &result);
                fill_report_success(&mut report, input.sizing, &result);
                report.warnings.extend(warnings);
            }
            Err(err) => {
                report.errors.push(err.to_string());
                apply_legacy_fallback(input.sizing, input.legacy_uniform_scale);
            }
        }
        return report;
    }

    if try_infer_metric_targets(&mut input, &mut report) {
        return report;
    }

    if let Some(legacy) = input.legacy_uniform_scale {
        input.sizing.calculated_baseline_scale = AuthoringScale::from_uniform_f32(legacy).ok();
        input.sizing.migration_state = SizingMigrationState::LegacyExplicitScale;
        report
            .warnings
            .push("using legacy explicit render scale — add Desired dimensions to migrate".into());
        if let Some(scale) = input.sizing.calculated_baseline_scale {
            report.quantized_baseline_scale = Some(scale);
            report.exact_calculated_scale = Some([legacy, legacy, legacy]);
        }
    } else {
        input.sizing.calculated_baseline_scale = Some(AuthoringScale::uniform_one());
        input.sizing.migration_state = SizingMigrationState::MissingSizingData;
        report.errors.push(
            "AT1 MissingSizingData: no Desired meters and no explicit baseline — using scale 1.0; author Desired*M or ExplicitBaselineScale and re-import"
                .into(),
        );
        report.warnings.push(
            "AT1: catalog lacks metric sizing — runtime presentation may be microscopic or enormous until migrated"
                .into(),
        );
    }

    report
}

fn try_infer_metric_targets(
    input: &mut SizingResolveInput<'_>,
    report: &mut AssetSizingReport,
) -> bool {
    if input.sizing.has_desired_dimensions() {
        return false;
    }

    let backup = input.sizing.clone();
    let inferred = match input.kind {
        ContentSizingKind::Unit => {
            let Some(height) = input
                .unit_height_hint_meters
                .filter(|height| *height > 0.0)
            else {
                return false;
            };
            input.sizing.desired_height_meters = Some(height);
            input.sizing.size_reference_axis = Some(SizeReferenceAxis::Height);
            "unit height hint"
        }
        ContentSizingKind::Doodad => {
            let Some(height) = input
                .doodad_kind_height_hint_meters
                .filter(|height| *height > 0.0)
            else {
                return false;
            };
            input.sizing.desired_height_meters = Some(height);
            input.sizing.size_reference_axis = Some(SizeReferenceAxis::Height);
            "doodad kind height"
        }
        ContentSizingKind::Building { .. } => {
            let (Some(width), Some(depth)) = (
                input.building_footprint_width_meters,
                input.building_footprint_depth_meters,
            ) else {
                return false;
            };
            apply_building_footprint_sizing_targets(input.sizing, width, depth);
            "building footprint"
        }
    };

    match resolve_from_desired_dimensions(input) {
        Ok((result, warnings)) => {
            apply_baseline_result(input.sizing, &result);
            fill_report_success(report, input.sizing, &result);
            report.warnings.extend(warnings);
            report.warnings.push(format!(
                "inferred desired meters from {inferred} — add Desired*M to Excel for explicit authoring"
            ));
            true
        }
        Err(err) => {
            *input.sizing = backup;
            report
                .warnings
                .push(format!("{inferred} sizing inference failed: {err}"));
            false
        }
    }
}

fn resolve_from_desired_dimensions(
    input: &mut SizingResolveInput<'_>,
) -> Result<(BaselineScaleResult, Vec<String>), AssetSizingError> {
    let path = asset_path_for_render_key(input.asset_root, input.render_key);
    let (mut source, origin, mut warnings) = measure_glb_source_bounds(
        &path,
        input.sizing.explicit_source_dimensions,
        input.sizing.source_bounds_node.as_deref(),
    )?;

    let (normalized, unit_note, unit_divisor) = normalize_source_dimensions_to_desired(
        source,
        input.sizing.desired_width_meters,
        input.sizing.desired_height_meters,
        input.sizing.desired_depth_meters,
    );
    source = normalized;
    input.sizing.source_bounds_unit_divisor = unit_divisor;
    if let Some(note) = unit_note {
        warnings.push(note);
    }

    input.sizing.calculated_source_bounds = Some(source);
    input.sizing.source_bounds_origin = Some(origin);

    if let Some(w) = check_suspected_unit_mismatch(
        source,
        input
            .sizing
            .desired_height_meters
            .or(input.sizing.desired_width_meters)
            .or(input.sizing.desired_depth_meters),
    ) {
        warnings.push(w);
    }

    let reference_axis = input.sizing.size_reference_axis.or(match input.kind {
        ContentSizingKind::Unit => Some(SizeReferenceAxis::Height),
        ContentSizingKind::Doodad => None,
        ContentSizingKind::Building { .. } => Some(SizeReferenceAxis::Height),
    });

    let desired_count = [
        input.sizing.desired_width_meters,
        input.sizing.desired_height_meters,
        input.sizing.desired_depth_meters,
    ]
    .into_iter()
    .filter(|v| v.is_some())
    .count();

    let policy = match input.kind {
        ContentSizingKind::Doodad if desired_count == 3 => SizingPolicy::DoodadNonUniform,
        ContentSizingKind::Building { .. } if desired_count == 3 => {
            SizingPolicy::DoodadNonUniform
        }
        ContentSizingKind::Doodad => SizingPolicy::ReferenceAxisUniform,
        _ => SizingPolicy::ReferenceAxisUniform,
    };

    let result = calculate_baseline_scale(
        policy,
        source,
        input.sizing.desired_width_meters,
        input.sizing.desired_height_meters,
        input.sizing.desired_depth_meters,
        reference_axis,
    )?;

    let quantized = quantize_baseline_scale(result.baseline_scale)?;
    let mut final_result = result;
    final_result.baseline_scale = quantized;

    if let ContentSizingKind::Building { safety_class } = input.kind {
        validate_building_topology(
            safety_class,
            input.building_footprint_width_meters,
            input.building_footprint_depth_meters,
            source,
            final_result.approximate_final_dimensions,
        )?;
    }

    input.sizing.migration_state = SizingMigrationState::MetricConfigured;
    Ok((final_result, warnings))
}

fn validate_building_topology(
    safety_class: BuildingTransformSafetyClass,
    footprint_width: Option<f32>,
    footprint_depth: Option<f32>,
    _source: SourceDimensions,
    final_dims: SourceDimensions,
) -> Result<(), AssetSizingError> {
    if !matches!(safety_class, BuildingTransformSafetyClass::Navigable) {
        return Ok(());
    }
    if let (Some(fw), Some(fd)) = (footprint_width, footprint_depth) {
        let width_delta = (final_dims.width_meters - fw).abs();
        let depth_delta = (final_dims.depth_meters - fd).abs();
        if width_delta > fw * 0.25 || depth_delta > fd * 0.25 {
            return Err(AssetSizingError::BuildingVisualTopologyScaleMismatch {
                message: format!(
                    "visual size ({:.2}×{:.2} m) diverges from footprint ({fw:.2}×{fd:.2} m)",
                    final_dims.width_meters, final_dims.depth_meters
                ),
            });
        }
    }
    Ok(())
}

fn apply_baseline_result(sizing: &mut AssetSizingDefinition, result: &BaselineScaleResult) {
    sizing.calculated_baseline_scale = Some(result.baseline_scale);
    sizing.migration_state = SizingMigrationState::MetricConfigured;
}

fn apply_legacy_fallback(sizing: &mut AssetSizingDefinition, legacy: Option<f32>) {
    sizing.calculated_baseline_scale = legacy
        .and_then(|v| AuthoringScale::from_uniform_f32(v).ok())
        .or(Some(AuthoringScale::uniform_one()));
    sizing.migration_state = if legacy.is_some() {
        SizingMigrationState::LegacyExplicitScale
    } else {
        SizingMigrationState::MissingSizingData
    };
}

fn fill_report_success(
    report: &mut AssetSizingReport,
    sizing: &AssetSizingDefinition,
    result: &BaselineScaleResult,
) {
    report.source_dimensions = sizing.calculated_source_bounds;
    report.source_bounds_origin = sizing.source_bounds_origin;
    report.exact_calculated_scale = Some(result.exact_scale_f32.to_array());
    report.quantized_baseline_scale = Some(result.baseline_scale);
    report.approximate_final_dimensions = Some(result.approximate_final_dimensions);
}

fn kind_label(kind: ContentSizingKind) -> &'static str {
    match kind {
        ContentSizingKind::Unit => "Unit",
        ContentSizingKind::Doodad => "Doodad",
        ContentSizingKind::Building { .. } => "Building",
    }
}

pub fn export_sizing_reports_markdown(
    path: &Path,
    reports: &[AssetSizingReport],
) -> std::io::Result<()> {
    use std::fs;
    use std::io::Write;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    writeln!(file, "# Asset Sizing Report")?;
    writeln!(file)?;
    for report in reports {
        writeln!(
            file,
            "## {} / {}",
            report.definition_kind, report.definition_id
        )?;
        writeln!(file, "- asset: `{}`", report.asset_path)?;
        if let Some(source) = report.source_dimensions {
            writeln!(
                file,
                "- source: {:.3} × {:.3} × {:.3} m ({:?})",
                source.width_meters,
                source.height_meters,
                source.depth_meters,
                report.source_bounds_origin
            )?;
        }
        if let Some(scale) = report.quantized_baseline_scale {
            let v = scale.to_vec3();
            writeln!(file, "- baseline scale: {:.3}, {:.3}, {:.3}", v.x, v.y, v.z)?;
        }
        for warning in &report.warnings {
            writeln!(file, "- warning: {warning}")?;
        }
        for error in &report.errors {
            writeln!(file, "- error: {error}")?;
        }
        writeln!(file)?;
    }
    Ok(())
}
