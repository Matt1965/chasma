//! Unit type catalog — authoritative definitions independent of world instances (ADR-027).

mod definition;
mod definition_id;
mod registry;
mod render_key;
mod starter;

pub use definition::UnitDefinition;
pub use definition_id::UnitDefinitionId;
pub use registry::{UnitCatalog, UnitCatalogError};
pub use render_key::UnitRenderKey;
#[cfg(test)]
pub use starter::starter_definitions;
