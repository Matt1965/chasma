mod definition;
mod definition_id;
mod registry;
#[cfg(test)]
mod starter;

pub use definition::ArmorProfileDefinition;
pub use definition_id::ArmorProfileId;
pub use registry::{ArmorProfileCatalog, ArmorProfileCatalogError};
#[cfg(test)]
pub use starter::starter_definitions;
