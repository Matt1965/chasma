//! Doodad type catalog — authoritative definitions independent of world instances (ADR-016).

mod definition;
mod definition_id;
mod registry;
mod render_key;
mod starter;

pub use definition::{DoodadDefinition, default_blocks_movement};
pub use definition_id::DoodadDefinitionId;
pub use registry::{DoodadCatalog, DoodadCatalogError};
pub use render_key::DoodadRenderKey;
#[cfg(test)]
pub use starter::starter_definitions;
