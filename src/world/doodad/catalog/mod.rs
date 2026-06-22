//! Doodad type catalog — authoritative definitions independent of world instances (ADR-016).

mod definition;
mod definition_id;
mod registry;
mod render_key;
mod starter;

pub use registry::{DoodadCatalog, DoodadCatalogError};
pub use definition::{default_blocks_movement, DoodadDefinition};
pub use definition_id::DoodadDefinitionId;
pub use render_key::DoodadRenderKey;
pub use starter::starter_definitions;
