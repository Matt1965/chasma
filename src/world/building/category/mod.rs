mod definition;
mod definition_id;
mod registry;
mod starter;

pub use definition::BuildingCategoryDefinition;
pub use definition_id::BuildingCategoryId;
pub use registry::{BuildingCategoryCatalog, BuildingCategoryCatalogError};
#[cfg(any(test, feature = "dev"))]
pub use starter::starter_definitions;
