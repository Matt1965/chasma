//! Offline asset sizing import helpers (ADR-097 DT1).

#[cfg(feature = "data-import")]
mod bounds;
#[cfg(feature = "data-import")]
mod columns;
#[cfg(feature = "data-import")]
mod pipeline;
#[cfg(feature = "data-import")]
mod targets;

#[cfg(feature = "data-import")]
pub use bounds::{
    DEFAULT_SIZE_REFERENCE_NODE, asset_path_for_render_key, measure_glb_source_bounds,
};
#[cfg(feature = "data-import")]
pub use columns::{
    AssetSizingColumns, BUILDING_ALLOW_INSTANCE_SCALE, BUILDING_DEFAULT_INSTANCE_SCALE_UNIFORM,
    BUILDING_MAX_UNIFORM_SCALE, BUILDING_MIN_UNIFORM_SCALE, BUILDING_TRANSFORM_SAFETY_CLASS,
    DESIRED_DEPTH_M, DESIRED_HEIGHT_M, DESIRED_WIDTH_M, DOODAD_ALLOW_NONUNIFORM_SCALE,
    DOODAD_BASE_COLLISION_RADIUS_X_M, DOODAD_BASE_COLLISION_RADIUS_Z_M, DOODAD_COLLISION_SHAPE,
    DOODAD_DEFAULT_INSTANCE_SCALE_X, DOODAD_DEFAULT_INSTANCE_SCALE_Y,
    DOODAD_DEFAULT_INSTANCE_SCALE_Z, DOODAD_GROUNDING_MODE, DOODAD_MAX_INSTANCE_SCALE,
    DOODAD_MIN_INSTANCE_SCALE, EXPLICIT_BASELINE_SCALE_UNIFORM, EXPLICIT_BASELINE_SCALE_X,
    EXPLICIT_BASELINE_SCALE_Y, EXPLICIT_BASELINE_SCALE_Z, MODEL_OFFSET_X_M, MODEL_OFFSET_Y_M,
    MODEL_OFFSET_Z_M, ROTATION_CORRECTION_X_DEG, ROTATION_CORRECTION_Y_DEG,
    ROTATION_CORRECTION_Z_DEG, SHARED_OPTIONAL_COLUMNS, SIZE_REFERENCE_AXIS, SOURCE_BOUNDS_NODE,
    SOURCE_DEPTH_M, SOURCE_HEIGHT_M, SOURCE_WIDTH_M, asset_sizing_from_columns,
    parse_asset_sizing_columns, parse_optional_f32,
};
#[cfg(feature = "data-import")]
pub use pipeline::{
    ContentSizingKind, SizingResolveInput, export_sizing_reports_markdown, resolve_content_sizing,
};
#[cfg(feature = "data-import")]
pub use targets::{
    apply_building_footprint_sizing_targets, unit_default_desired_height_meters,
};
