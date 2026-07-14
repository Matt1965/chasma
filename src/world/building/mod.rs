pub mod catalog;
pub mod category;
pub mod footprint;

mod authoring;
mod construction;
pub mod container_access;
mod id;
mod insert;
mod interaction_profile;
mod interior;
pub mod inventory;
pub mod inventory_error;
mod ownership;
mod placement;
mod placement_validation;
mod rebuild;
mod record;
mod restore;
mod source;
mod state;
mod store;
mod vitals;

pub use authoring::{
    BuildingAuthoringError, create_building, create_building_with_inventory, lookup_building,
    move_building, place_player_building, place_player_building_with_inventory, remove_building,
};
#[cfg(any(test, feature = "dev"))]
pub use catalog::starter_definitions;
pub use catalog::{
    BuildingCatalog, BuildingCatalogError, BuildingDefinition, BuildingDefinitionId,
    BuildingRenderKey,
};
#[cfg(any(test, feature = "dev"))]
pub use category::starter_definitions as starter_building_category_definitions;
pub use category::{
    BuildingCategoryCatalog, BuildingCategoryCatalogError, BuildingCategoryDefinition,
    BuildingCategoryId,
};
pub use construction::{
    BuildingConstructionReport, BuildingConstructionSettings, BuildingLifecycleError,
    BuildingLifecycleEvent, add_building_construction_progress, damage_building, destroy_building,
    heal_building, is_building_operational, set_building_lifecycle_stage,
    step_all_building_construction, transition_to_ruins,
};
pub use container_access::{
    ContainerAccessPolicy, InventoryAccessDenialReason, InventoryAccessResult,
};
pub use footprint::{FootprintSpec, FootprintType};
pub use id::BuildingId;
pub use insert::BuildingInsertError;
pub use interaction_profile::{
    BuildingCapabilities, BuildingInteractionProfile, BuildingInteractionProfileCatalog,
    INTERACTION_WORK_RANGE_METERS, InteractionPointDefinition, interaction_point_world_position,
    starter_interaction_profiles,
};
pub use interior::{
    DoorAccessPolicy, DoorId, DoorRecord, DoorState, DoorStore, InteriorError,
    InteriorProfileCatalog, InteriorProfileId, activate_building_interior, close_door,
    deactivate_building_interior, destroy_door, lock_door, open_door, portal_traversable,
    space_route_for_unit, starter_interior_profiles, try_open_door_at_portal_for_unit,
    try_open_door_for_unit, two_story_hut_interior_profile,
};
pub use inventory::{
    BuildingInventoryContext, BuildingInventoryRemovalPolicy, attach_inventory_on_building_create,
    building_container_access_policy, building_inventory_operational,
    can_unit_access_building_inventory, can_unit_access_inventory,
    cleanup_building_inventory_on_delete, create_building_inventory,
    finalize_building_inventory_removal, set_building_container_locked, spill_building_inventory,
    spill_position_for_building, validate_building_inventory_links,
    validate_building_inventory_owner,
};
pub use inventory_error::BuildingInventoryError;
pub use ownership::BuildingOwnership;
pub use placement::BuildingPlacement;
pub use placement_validation::{
    BuildingPlacementConfig, BuildingPlacementContext, BuildingPlacementRejectReason,
    BuildingPlacementValidation, anchor_from_terrain_position, rotation_from_quadrants,
    snap_anchor_global_xz, validate_building_placement,
};
pub use rebuild::{BuildingRebuildError, rebuild_building_world_indexes};
pub use record::BuildingRecord;
pub use restore::{BuildingRestoreError, validate_building_for_restore};
pub use source::BuildingSource;
pub use state::{BuildingInteriorState, BuildingLifecycleState, BuildingSpaces, ConstructionState};
pub use store::ChunkBuildingStore;
pub use vitals::BuildingVitals;
