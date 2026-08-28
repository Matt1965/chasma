mod definition;
mod id;
mod registry;
mod starter;

pub use definition::SpeciesDefinition;
pub use id::SpeciesId;
pub use registry::{SpeciesCatalog, SpeciesCatalogError};
#[cfg(test)]
pub use starter::starter_definitions;
