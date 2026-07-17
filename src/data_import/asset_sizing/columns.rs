//! Shared asset-sizing Excel column names (ADR-097 DT1).

pub const DESIRED_WIDTH_M: &str = "Desired Width M";
pub const DESIRED_HEIGHT_M: &str = "Desired Height M";
pub const DESIRED_DEPTH_M: &str = "Desired Depth M";
pub const SIZE_REFERENCE_AXIS: &str = "Size Reference Axis";
pub const SOURCE_BOUNDS_NODE: &str = "Source Bounds Node";
pub const SOURCE_WIDTH_M: &str = "Source Width M";
pub const SOURCE_HEIGHT_M: &str = "Source Height M";
pub const SOURCE_DEPTH_M: &str = "Source Depth M";
pub const MODEL_OFFSET_X_M: &str = "Model Offset X M";
pub const MODEL_OFFSET_Y_M: &str = "Model Offset Y M";
pub const MODEL_OFFSET_Z_M: &str = "Model Offset Z M";
pub const ROTATION_CORRECTION_X_DEG: &str = "Rotation Correction X Deg";
pub const ROTATION_CORRECTION_Y_DEG: &str = "Rotation Correction Y Deg";
pub const ROTATION_CORRECTION_Z_DEG: &str = "Rotation Correction Z Deg";
pub const EXPLICIT_BASELINE_SCALE_X: &str = "Explicit Baseline Scale X";
pub const EXPLICIT_BASELINE_SCALE_Y: &str = "Explicit Baseline Scale Y";
pub const EXPLICIT_BASELINE_SCALE_Z: &str = "Explicit Baseline Scale Z";
pub const EXPLICIT_BASELINE_SCALE_UNIFORM: &str = "Explicit Baseline Scale Uniform";

pub const DOODAD_DEFAULT_INSTANCE_SCALE_X: &str = "Default Instance Scale X";
pub const DOODAD_DEFAULT_INSTANCE_SCALE_Y: &str = "Default Instance Scale Y";
pub const DOODAD_DEFAULT_INSTANCE_SCALE_Z: &str = "Default Instance Scale Z";
pub const DOODAD_ALLOW_NONUNIFORM_SCALE: &str = "Allow Nonuniform Scale";
pub const DOODAD_MIN_INSTANCE_SCALE: &str = "Min Instance Scale";
pub const DOODAD_MAX_INSTANCE_SCALE: &str = "Max Instance Scale";
pub const DOODAD_COLLISION_SHAPE: &str = "Collision Shape";
pub const DOODAD_BASE_COLLISION_RADIUS_X_M: &str = "Base Collision Radius X M";
pub const DOODAD_BASE_COLLISION_RADIUS_Z_M: &str = "Base Collision Radius Z M";
pub const DOODAD_GROUNDING_MODE: &str = "Grounding Mode";

pub const BUILDING_DEFAULT_INSTANCE_SCALE_UNIFORM: &str = "Default Instance Scale Uniform";
pub const BUILDING_ALLOW_INSTANCE_SCALE: &str = "Allow Instance Scale";
pub const BUILDING_MIN_UNIFORM_SCALE: &str = "Min Uniform Scale";
pub const BUILDING_MAX_UNIFORM_SCALE: &str = "Max Uniform Scale";
pub const BUILDING_TRANSFORM_SAFETY_CLASS: &str = "Building Transform Safety Class";

pub const SHARED_OPTIONAL_COLUMNS: &[&str] = &[
    DESIRED_WIDTH_M,
    DESIRED_HEIGHT_M,
    DESIRED_DEPTH_M,
    SIZE_REFERENCE_AXIS,
    SOURCE_BOUNDS_NODE,
    SOURCE_WIDTH_M,
    SOURCE_HEIGHT_M,
    SOURCE_DEPTH_M,
    MODEL_OFFSET_X_M,
    MODEL_OFFSET_Y_M,
    MODEL_OFFSET_Z_M,
    ROTATION_CORRECTION_X_DEG,
    ROTATION_CORRECTION_Y_DEG,
    ROTATION_CORRECTION_Z_DEG,
    EXPLICIT_BASELINE_SCALE_X,
    EXPLICIT_BASELINE_SCALE_Y,
    EXPLICIT_BASELINE_SCALE_Z,
    EXPLICIT_BASELINE_SCALE_UNIFORM,
];

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::asset_sizing::AssetSizingDefinition;
use crate::world::authoring_transform::{AuthoringScale, QuantizedOrientation};

/// Parsed sizing columns from one worksheet row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssetSizingColumns {
    pub desired_width_meters: Option<f32>,
    pub desired_height_meters: Option<f32>,
    pub desired_depth_meters: Option<f32>,
    pub size_reference_axis: Option<String>,
    pub source_bounds_node: Option<String>,
    pub source_width_meters: Option<f32>,
    pub source_height_meters: Option<f32>,
    pub source_depth_meters: Option<f32>,
    pub model_offset: Vec3,
    pub rotation_correction_degrees: Option<(f32, f32, f32)>,
    pub explicit_baseline_scale_xyz: Option<(f32, f32, f32)>,
    pub explicit_baseline_scale_uniform: Option<f32>,
}

pub fn parse_optional_f32(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse().ok()
    }
}

pub fn parse_asset_sizing_columns(
    columns: &HashMap<String, usize>,
    cells: &[calamine::Data],
    text: &dyn Fn(&str) -> String,
) -> AssetSizingColumns {
    let offset = |x_col: &str, y_col: &str, z_col: &str| -> Vec3 {
        Vec3::new(
            columns
                .get(x_col)
                .and_then(|&i| cells.get(i))
                .map(|c| cell_to_string(c))
                .and_then(|s| parse_optional_f32(&s))
                .unwrap_or(0.0),
            columns
                .get(y_col)
                .and_then(|&i| cells.get(i))
                .map(|c| cell_to_string(c))
                .and_then(|s| parse_optional_f32(&s))
                .unwrap_or(0.0),
            columns
                .get(z_col)
                .and_then(|&i| cells.get(i))
                .map(|c| cell_to_string(c))
                .and_then(|s| parse_optional_f32(&s))
                .unwrap_or(0.0),
        )
    };

    let explicit_xyz = {
        let x = parse_optional_f32(&text(EXPLICIT_BASELINE_SCALE_X));
        let y = parse_optional_f32(&text(EXPLICIT_BASELINE_SCALE_Y));
        let z = parse_optional_f32(&text(EXPLICIT_BASELINE_SCALE_Z));
        match (x, y, z) {
            (Some(x), Some(y), Some(z)) => Some((x, y, z)),
            (None, None, None) => None,
            _ => Some((x.unwrap_or(1.0), y.unwrap_or(1.0), z.unwrap_or(1.0))),
        }
    };

    let rot = {
        let x = parse_optional_f32(&text(ROTATION_CORRECTION_X_DEG));
        let y = parse_optional_f32(&text(ROTATION_CORRECTION_Y_DEG));
        let z = parse_optional_f32(&text(ROTATION_CORRECTION_Z_DEG));
        if x.is_none() && y.is_none() && z.is_none() {
            None
        } else {
            Some((x.unwrap_or(0.0), y.unwrap_or(0.0), z.unwrap_or(0.0)))
        }
    };

    AssetSizingColumns {
        desired_width_meters: parse_optional_f32(&text(DESIRED_WIDTH_M)),
        desired_height_meters: parse_optional_f32(&text(DESIRED_HEIGHT_M)),
        desired_depth_meters: parse_optional_f32(&text(DESIRED_DEPTH_M)),
        size_reference_axis: {
            let raw = text(SIZE_REFERENCE_AXIS);
            if raw.trim().is_empty() {
                None
            } else {
                Some(raw)
            }
        },
        source_bounds_node: {
            let raw = text(SOURCE_BOUNDS_NODE);
            if raw.trim().is_empty() {
                None
            } else {
                Some(raw)
            }
        },
        source_width_meters: parse_optional_f32(&text(SOURCE_WIDTH_M)),
        source_height_meters: parse_optional_f32(&text(SOURCE_HEIGHT_M)),
        source_depth_meters: parse_optional_f32(&text(SOURCE_DEPTH_M)),
        model_offset: offset(MODEL_OFFSET_X_M, MODEL_OFFSET_Y_M, MODEL_OFFSET_Z_M),
        rotation_correction_degrees: rot,
        explicit_baseline_scale_xyz: explicit_xyz,
        explicit_baseline_scale_uniform: parse_optional_f32(&text(EXPLICIT_BASELINE_SCALE_UNIFORM)),
    }
}

pub fn asset_sizing_from_columns(
    columns: &AssetSizingColumns,
) -> Result<AssetSizingDefinition, String> {
    let rotation_correction = if let Some((x, y, z)) = columns.rotation_correction_degrees {
        QuantizedOrientation::from_degrees(x, y, z)
            .map_err(|_| "invalid rotation correction degrees".to_string())?
    } else {
        QuantizedOrientation::IDENTITY
    };

    let explicit_baseline_scale = if let Some(uniform) = columns.explicit_baseline_scale_uniform {
        Some(
            AuthoringScale::from_uniform_f32(uniform)
                .map_err(|_| "explicit baseline uniform scale out of range".to_string())?,
        )
    } else if let Some((x, y, z)) = columns.explicit_baseline_scale_xyz {
        Some(
            AuthoringScale::from_non_uniform_f32(x, y, z)
                .map_err(|_| "explicit baseline XYZ scale out of range".to_string())?,
        )
    } else {
        None
    };

    let explicit_source_dimensions = match (
        columns.source_width_meters,
        columns.source_height_meters,
        columns.source_depth_meters,
    ) {
        (Some(w), Some(h), Some(d)) => Some(crate::world::asset_sizing::SourceDimensions {
            width_meters: w,
            height_meters: h,
            depth_meters: d,
        }),
        (None, None, None) => None,
        _ => {
            return Err("Source Width/Height/Depth M must all be provided together".to_string());
        }
    };

    Ok(AssetSizingDefinition {
        desired_width_meters: columns.desired_width_meters,
        desired_height_meters: columns.desired_height_meters,
        desired_depth_meters: columns.desired_depth_meters,
        size_reference_axis: columns
            .size_reference_axis
            .as_deref()
            .and_then(crate::world::asset_sizing::SizeReferenceAxis::parse),
        source_bounds_node: columns.source_bounds_node.clone(),
        explicit_source_dimensions,
        model_local_offset_meters: columns.model_offset,
        rotation_correction,
        explicit_baseline_scale,
        ..AssetSizingDefinition::default()
    })
}

fn cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::String(s) => s.clone(),
        calamine::Data::Float(f) => f.to_string(),
        calamine::Data::Int(i) => i.to_string(),
        calamine::Data::Bool(b) => b.to_string(),
        calamine::Data::DateTime(dt) => dt.to_string(),
        calamine::Data::DateTimeIso(s) => s.clone(),
        calamine::Data::DurationIso(s) => s.clone(),
        calamine::Data::Error(e) => format!("{e:?}"),
        calamine::Data::Empty => String::new(),
    }
}
