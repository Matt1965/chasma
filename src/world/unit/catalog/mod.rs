//! Unit type catalog — authoritative definitions independent of world instances (ADR-027).

mod definition;
mod definition_id;
mod inventory_profile;
mod registry;
mod render_key;
mod starter;
mod work;

pub use definition::UnitDefinition;
pub use definition_id::UnitDefinitionId;
pub use inventory_profile::{
    UnitInventoryProfileValidationError, validate_unit_inventory_profile_reference,
};
pub use registry::{UnitCatalog, UnitCatalogError};
pub use render_key::UnitRenderKey;
#[cfg(any(test, feature = "dev"))]
pub use starter::starter_definitions;
pub use work::UnitWorkCapabilities;
