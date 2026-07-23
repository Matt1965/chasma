mod definition;
mod definition_id;
mod registry;
mod render_key;
mod revision;
mod starter;
mod variant;

pub use definition::BuildingDefinition;
pub use definition_id::BuildingDefinitionId;
pub use registry::{BuildingCatalog, BuildingCatalogError};
pub use revision::BuildingCatalogRevision;
pub use render_key::BuildingRenderKey;
pub use variant::{
    BuildingVariantCreateInput, BuildingVariantCreateOutcome, create_building_variant,
    export_building_catalog_snapshot, replace_building_instance_definition,
    suggest_variant_definition_id, validate_building_definition_id,
};
#[cfg(any(test, feature = "dev"))]
pub use starter::starter_definitions;
