use bevy::prelude::*;

use super::definition_id::BuildingDefinitionId;
use super::render_key::BuildingRenderKey;
use crate::world::AnimationProfileId;
use crate::world::building::category::BuildingCategoryId;
use crate::world::building::footprint::{FootprintSpec, FootprintType};

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
    /// Reserved construction stage set id (B4+).
    pub construction_stages_ref: Option<String>,
    /// Reserved task-generation profile id (ADR-072).
    pub task_provider_id: Option<String>,
    pub animation_profile_id: Option<AnimationProfileId>,
    /// Reserved interaction profile id (ADR-042 extension).
    pub interaction_profile_id: Option<String>,
    /// Reserved default navigable space id (B6+).
    pub default_space_id: Option<String>,
    /// Interior profile reference (ADR-084 B7).
    pub interior_profile_id: Option<String>,
    pub max_slope_degrees: f32,
    pub enabled: bool,
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
            construction_stages_ref: None,
            task_provider_id: None,
            animation_profile_id: None,
            interaction_profile_id: None,
            default_space_id: None,
            interior_profile_id: None,
            max_slope_degrees,
            enabled,
        }
    }

    pub fn with_footprint_id(mut self, footprint_id: crate::world::FootprintId) -> Self {
        self.footprint_id = Some(footprint_id);
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
}
