mod assess;
mod ensure;
mod error;
mod operation_scope;
mod rebuild;
mod revision;
mod sample_cells;
mod store;
mod types;

pub use assess::{
    assess_building_terrain, assess_building_terrain_at_placement, format_coverage_display,
    format_efficiency_display, format_field_average_display,
};
pub use ensure::{assessment_revision_fingerprint, ensure_building_terrain_assessment};
pub use error::{TerrainAssessmentCatalogs, TerrainAssessmentError, TerrainAssessmentUiError};
pub use operation_scope::{
    OperationScopedTerrainEfficiency, terrain_efficiency_for_operation,
};
pub use rebuild::{
    AssessmentRebuildOutcome, AssessmentRebuildReport, invalidate_buildings_for_changed_fields,
    rebuild_all_building_terrain_assessments, rebuild_building_terrain_assessment,
};
pub use revision::{BuildingTerrainAssessmentKey, hash_sample_cells};
pub use sample_cells::resolve_building_field_sample_cells;
pub use store::BuildingTerrainAssessmentStore;
pub use types::{
    BuildingFieldRequirementAssessment, BuildingTerrainAssessment, BuildingTerrainWarning,
    FieldTileRevisionEntry, RequirementAssessmentAvailability,
};
