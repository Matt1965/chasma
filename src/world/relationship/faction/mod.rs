mod definition;
mod id;
mod registry;
mod starter;

pub use definition::FactionDefinition;
pub use id::FactionId;
pub use registry::{FactionCatalog, FactionCatalogError};
#[cfg(test)]
pub use starter::starter_definitions;
