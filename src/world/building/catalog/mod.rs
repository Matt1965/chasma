mod definition;
mod definition_id;
mod registry;
mod render_key;
mod starter;

pub use definition::BuildingDefinition;
pub use definition_id::BuildingDefinitionId;
pub use registry::{BuildingCatalog, BuildingCatalogError};
pub use render_key::BuildingRenderKey;
#[cfg(any(test, feature = "dev"))]
pub use starter::starter_definitions;
