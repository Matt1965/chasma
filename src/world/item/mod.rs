//! Item definition data layer (ADR-087 I1).
//!
//! Owns item type definitions and categories in catalogs. Runtime inventory
//! placement is deferred to later phases.

mod catalog;
mod category;
mod category_id;
mod definition;
mod definition_id;
mod icon_key;
mod render_key;
mod validation;

#[cfg(any(test, feature = "dev"))]
pub use catalog::starter_definitions;
pub use catalog::{ItemCatalog, ItemCatalogError};
#[cfg(any(test, feature = "dev"))]
pub use category::starter_definitions as starter_item_category_definitions;
pub use category::{ItemCategoryCatalog, ItemCategoryCatalogError, ItemCategoryDefinition};
pub use category_id::ItemCategoryId;
pub use definition::ItemDefinition;
pub use definition_id::ItemDefinitionId;
pub use icon_key::ItemIconKey;
pub use render_key::ItemRenderKey;
pub use validation::{
    ItemValidationError, MAX_ITEM_GRID_DIMENSION, normalize_tags, validate_item_definition,
};
