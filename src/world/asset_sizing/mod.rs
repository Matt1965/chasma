//! Metric GLB asset sizing contract (ADR-097 DT1).

mod baseline;
mod composition;
mod definition;
mod error;
mod finalize;
mod report;

pub use baseline::{
    BaselineScaleResult, SizingPolicy, calculate_baseline_scale, check_suspected_unit_mismatch,
    quantize_baseline_scale,
};
pub use composition::{
    building_baseline_render_scale, building_effective_model_offset,
    building_model_child_local_transform, building_model_child_scale, building_uses_model_child,
    doodad_baseline_render_scale, doodad_final_render_scale,
    doodad_visual_collision_mismatch_warning, sizing_rotation_correction,
    unit_baseline_render_scale,
};
pub use definition::{
    AssetSizingDefinition, DoodadCollisionShape, DoodadGroundingMode, SizeReferenceAxis,
    SizingMigrationState, SourceBoundsOrigin, SourceDimensions,
};
pub use error::AssetSizingError;
pub use finalize::{
    finalize_building_definition, finalize_doodad_definition, finalize_unit_definition,
};
pub use report::{AssetSizingReport, sort_reports};
