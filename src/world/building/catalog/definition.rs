use bevy::prelude::*;

use super::definition_id::BuildingDefinitionId;
use super::render_key::BuildingRenderKey;
use crate::world::AnimationProfileId;
use crate::world::InventoryProfileId;
use crate::world::asset_sizing::AssetSizingDefinition;
use crate::world::authoring_transform::BuildingTransformSafetyClass;
use crate::world::building::category::BuildingCategoryId;
use crate::world::building::container_access::ContainerAccessPolicy;
use crate::world::building::footprint::{FootprintSpec, FootprintType};
use crate::world::building::inventory_binding::{
    BuildingInventoryBindingDefinition, BuildingInventoryBindingId,
};
use crate::world::operation::OperationDefinitionId;

/// Authoritative description of a building type (ADR-078 B1).
///
/// Catalog definitions are independent of world instances, ECS, rendering, and
/// occupancy runtime. Instance records arrive in a later phase.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingDefinition {
    pub id: BuildingDefinitionId,
    pub display_name: String,
    pub category_id: BuildingCategoryId,
    /// Primary visual model asset key (`assets/buildings/{key}.glb`).
    pub render_key: BuildingRenderKey,
    /// Collision mesh input for future offline occupancy baking.
    pub collision_render_key: BuildingRenderKey,
    /// Optional alternate preview mesh for placement ghosts (later phases).
    pub preview_render_key: Option<BuildingRenderKey>,
    pub max_hp: u32,
    /// Target construction duration in seconds (authoring baseline).
    pub build_time_seconds: f32,
    pub footprint_type: FootprintType,
    pub footprint: FootprintSpec,
    /// Optional catalog footprint reference. When unset, inline [`Self::footprint`] is used.
    pub footprint_id: Option<crate::world::FootprintId>,
    /// Default operational terrain-field sampling footprint when requirements do not override.
    pub field_sampling_footprint_id: Option<crate::world::FootprintId>,
    /// Reserved construction stage set id (B4+).
    pub construction_stages_ref: Option<String>,
    /// Reserved task-generation profile id (ADR-072). Deprecated — use [`Self::supported_operations`].
    pub task_provider_id: Option<String>,
    /// Explicit supported production operations (EP3).
    pub supported_operations: Vec<OperationDefinitionId>,
    /// Authored default operation when multiple are supported (EP3).
    pub default_operation_id: Option<OperationDefinitionId>,
    pub animation_profile_id: Option<AnimationProfileId>,
    /// Reserved interaction profile id (ADR-042 extension).
    pub interaction_profile_id: Option<String>,
    /// Reserved default navigable space id (B6+).
    pub default_space_id: Option<String>,
    /// Interior profile reference (ADR-084 B7).
    pub interior_profile_id: Option<String>,
    pub max_slope_degrees: f32,
    pub enabled: bool,
    /// Optional inventory container profile (ADR-087 I1). None = no inventory.
    pub inventory_profile_id: Option<InventoryProfileId>,
    /// Who may access this container at runtime (ADR-091 I5).
    pub inventory_access_policy: ContainerAccessPolicy,
    /// Interaction point key for container access/spill placement (ADR-091 I5).
    pub inventory_interaction_point_key: Option<String>,
    /// When true, destruction spills surviving contents to world piles (ADR-091 I5).
    pub spill_on_destroy: bool,
    /// Role-tagged inventory layout (EP4). When empty, [`Self::inventory_profile_id`] migrates as `primary`.
    pub inventory_bindings: Vec<BuildingInventoryBindingDefinition>,
    /// Explicit default binding for generic container access (EP4).
    pub default_inventory_binding_id: Option<BuildingInventoryBindingId>,
    /// Visual model offset from authoritative anchor in local space (ADR-096 BP-CLEANUP).
    pub model_local_offset: Vec3,
    /// Additional yaw correction (degrees) applied to render root only (ADR-096).
    pub model_yaw_correction_degrees: f32,
    /// Metric asset sizing metadata (ADR-097 DT1).
    pub asset_sizing: AssetSizingDefinition,
    pub transform_safety_class: BuildingTransformSafetyClass,
    /// Dev-only instance uniform scale seam (player Build Mode uses definition default).
    pub allow_instance_scale: bool,
    pub min_uniform_instance_scale: f32,
    pub max_uniform_instance_scale: f32,
    /// Authored logistics routes for hauling request generation (EP7).
    pub logistics_routes: Vec<crate::world::logistics::BuildingLogisticsRouteDefinition>,
}

impl BuildingDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: BuildingDefinitionId,
        display_name: impl Into<String>,
        category_id: BuildingCategoryId,
        render_key: BuildingRenderKey,
        collision_render_key: BuildingRenderKey,
        max_hp: u32,
        build_time_seconds: f32,
        footprint: FootprintSpec,
        max_slope_degrees: f32,
        enabled: bool,
    ) -> Self {
        let footprint_type = footprint.footprint_type();
        Self {
            id,
            display_name: display_name.into(),
            category_id,
            render_key,
            collision_render_key,
            preview_render_key: None,
            max_hp,
            build_time_seconds,
            footprint_type,
            footprint,
            footprint_id: None,
            field_sampling_footprint_id: None,
            construction_stages_ref: None,
            task_provider_id: None,
            supported_operations: Vec::new(),
            default_operation_id: None,
            animation_profile_id: None,
            interaction_profile_id: None,
            default_space_id: None,
            interior_profile_id: None,
            max_slope_degrees,
            enabled,
            inventory_profile_id: None,
            inventory_access_policy: ContainerAccessPolicy::OwnerOnly,
            inventory_interaction_point_key: None,
            spill_on_destroy: true,
            inventory_bindings: Vec::new(),
            default_inventory_binding_id: None,
            model_local_offset: Vec3::ZERO,
            model_yaw_correction_degrees: 0.0,
            asset_sizing: AssetSizingDefinition::default(),
            transform_safety_class: BuildingTransformSafetyClass::Navigable,
            allow_instance_scale: false,
            min_uniform_instance_scale: 0.05,
            max_uniform_instance_scale: 20.0,
            logistics_routes: Vec::new(),
        }
    }

    pub fn model_yaw_correction_radians(&self) -> f32 {
        self.model_yaw_correction_degrees.to_radians()
    }

    pub fn with_model_local_offset(mut self, offset: Vec3) -> Self {
        self.model_local_offset = offset;
        self
    }

    pub fn with_model_yaw_correction_degrees(mut self, degrees: f32) -> Self {
        self.model_yaw_correction_degrees = degrees;
        self
    }

    pub fn with_footprint_id(mut self, footprint_id: crate::world::FootprintId) -> Self {
        self.footprint_id = Some(footprint_id);
        self
    }

    pub fn with_field_sampling_footprint_id(
        mut self,
        footprint_id: crate::world::FootprintId,
    ) -> Self {
        self.field_sampling_footprint_id = Some(footprint_id);
        self
    }

    pub fn with_preview_render_key(mut self, preview_render_key: BuildingRenderKey) -> Self {
        self.preview_render_key = Some(preview_render_key);
        self
    }

    pub fn with_construction_stages_ref(mut self, value: impl Into<String>) -> Self {
        self.construction_stages_ref = Some(value.into());
        self
    }

    pub fn with_task_provider_id(mut self, value: impl Into<String>) -> Self {
        self.task_provider_id = Some(value.into());
        self
    }

    pub fn with_supported_operations(
        mut self,
        operations: impl IntoIterator<Item = OperationDefinitionId>,
    ) -> Self {
        self.supported_operations = operations.into_iter().collect();
        self
    }

    pub fn with_default_operation_id(mut self, operation_id: OperationDefinitionId) -> Self {
        self.default_operation_id = Some(operation_id);
        self
    }

    pub fn supports_operation(&self, operation_id: &OperationDefinitionId) -> bool {
        self.supported_operations
            .iter()
            .any(|supported| supported == operation_id)
    }

    /// Resolve the authored or implicit default operation (EP3).
    ///
    /// Never chooses the first supported operation when multiple exist without an authored default.
    pub fn resolved_default_operation(&self) -> Option<OperationDefinitionId> {
        if let Some(default_id) = &self.default_operation_id {
            if self.supports_operation(default_id) {
                return Some(default_id.clone());
            }
            return None;
        }
        if self.supported_operations.len() == 1 {
            return Some(self.supported_operations[0].clone());
        }
        None
    }

    pub fn with_animation_profile_id(mut self, profile_id: AnimationProfileId) -> Self {
        self.animation_profile_id = Some(profile_id);
        self
    }

    pub fn with_interaction_profile_id(mut self, value: impl Into<String>) -> Self {
        self.interaction_profile_id = Some(value.into());
        self
    }

    pub fn with_default_space_id(mut self, value: impl Into<String>) -> Self {
        self.default_space_id = Some(value.into());
        self
    }

    pub fn with_interior_profile_id(mut self, value: impl Into<String>) -> Self {
        self.interior_profile_id = Some(value.into());
        self
    }

    pub fn with_inventory_profile_id(mut self, profile_id: InventoryProfileId) -> Self {
        self.inventory_profile_id = Some(profile_id);
        self
    }

    pub fn with_inventory_access_policy(mut self, policy: ContainerAccessPolicy) -> Self {
        self.inventory_access_policy = policy;
        self
    }

    pub fn with_inventory_interaction_point_key(mut self, key: impl Into<String>) -> Self {
        self.inventory_interaction_point_key = Some(key.into());
        self
    }

    pub fn with_spill_on_destroy(mut self, spill_on_destroy: bool) -> Self {
        self.spill_on_destroy = spill_on_destroy;
        self
    }

    pub fn with_inventory_bindings(
        mut self,
        bindings: impl IntoIterator<Item = BuildingInventoryBindingDefinition>,
    ) -> Self {
        self.inventory_bindings = bindings.into_iter().collect();
        self
    }

    pub fn with_default_inventory_binding_id(
        mut self,
        binding_id: BuildingInventoryBindingId,
    ) -> Self {
        self.default_inventory_binding_id = Some(binding_id);
        self
    }

    pub fn with_allow_instance_scale(mut self, allow: bool) -> Self {
        self.allow_instance_scale = allow;
        self
    }

    pub fn with_logistics_routes(
        mut self,
        routes: impl IntoIterator<Item = crate::world::logistics::BuildingLogisticsRouteDefinition>,
    ) -> Self {
        self.logistics_routes = routes.into_iter().collect();
        self
    }
}
