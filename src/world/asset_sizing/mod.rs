mod authority;
mod baseline;
mod composition;
mod definition;
mod error;
mod finalize;
mod report;

pub use authority::{
    normalize_building_sizing_authority, sync_building_legacy_mirrors_from_sizing,
    validate_building_sizing_authority, validate_sizing_migration_state, SizingAuthorityIssue,
};
pub use baseline::{
    BaselineScaleResult, SizingPolicy, calculate_baseline_scale, check_suspected_unit_mismatch,
    normalize_source_dimensions_to_desired, quantize_baseline_scale,
};
pub use composition::{
    building_baseline_render_scale, building_effective_model_offset,
    building_model_child_local_transform, building_model_child_scale, building_uses_model_child,
    building_visual_footprint_mismatch_warning, building_visual_scale, compose_visual_scale,
    definition_visual_baseline, doodad_baseline_render_scale, doodad_final_render_scale,
    doodad_visual_collision_mismatch_warning, doodad_visual_scale, sizing_rotation_correction,
    unit_baseline_render_scale, unit_visual_scale,
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
