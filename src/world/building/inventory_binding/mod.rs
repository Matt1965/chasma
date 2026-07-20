//! Role-tagged building inventory bindings (EP4).

mod binding;
mod binding_id;
mod definition;
mod query;
mod role;
mod store;
mod validation;

#[cfg(test)]
mod tests;

pub use binding::BuildingInventoryBinding;
pub use binding_id::BuildingInventoryBindingId;
pub use definition::BuildingInventoryBindingDefinition;
pub use query::{
    building_inventories_with_role, building_inventory_bindings, default_building_inventory_binding,
    primary_building_inventory_id, resolve_building_inventory_binding,
};
pub use role::BuildingInventoryRole;
pub use store::{BuildingInventoryBindingSet, BuildingInventoryBindingStore};
pub use validation::{
    BuildingInventoryBindingValidationIssue, effective_inventory_binding_definitions,
    validate_building_catalog_inventory_bindings,
    validate_building_definition_inventory_bindings,
    validate_building_runtime_inventory_bindings, validate_operation_inventory_bindings,
    validate_selected_operation_inventory_bindings, validate_world_building_inventory_bindings,
};
