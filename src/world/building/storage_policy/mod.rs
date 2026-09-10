//! Per-building inbound storage acceptance policy and queries.

mod api;
mod player_policy;
mod policy;
mod store;

#[cfg(test)]
mod tests;

pub use api::{
    all_storage_filter_categories, binding_is_production_output_surplus_source,
    building_is_production_output_surplus_source, building_is_storage_capable,
    building_storage_accepts_item, building_storage_accepts_item_for_definition,
    building_storage_policy, default_storage_delivery_binding_id,
    mark_settlement_storage_logistics_wakeup, mark_storage_logistics_dirty_for_inventory,
    storage_delivery_inventory_ids, storage_item_is_misfiled, storage_policy_accepts_category,
};
pub use player_policy::{
    apply_player_storage_accept_all, apply_player_storage_category_accepted,
    apply_player_storage_clear_all, effective_storage_category_accepted,
};
pub use policy::BuildingStoragePolicy;
pub use store::{BuildingStoragePolicySaveState, BuildingStoragePolicyStore};
