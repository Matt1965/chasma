mod registry;
mod starter;

pub use registry::{InventoryProfileCatalog, InventoryProfileCatalogError};
#[cfg(any(test, feature = "dev"))]
pub use starter::starter_definitions;
