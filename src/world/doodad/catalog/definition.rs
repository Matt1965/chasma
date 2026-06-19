use bevy::prelude::*;

use super::definition_id::DoodadDefinitionId;
use super::render_key::DoodadRenderKey;
use crate::world::DoodadKind;

/// Authoritative description of a doodad type (ADR-016).
///
/// Catalog definitions are independent of world instances, ECS, rendering, and
/// terrain runtime. [`DoodadRecord`] instances reference [`DoodadDefinitionId`]
/// as the authoritative type (ADR-017); [`DoodadKind`] is cached on the record.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct DoodadDefinition {
    pub id: DoodadDefinitionId,
    pub kind: DoodadKind,
    pub display_name: String,
    /// Minimum center-to-center spacing when placing instances (meters).
    pub placement_radius_meters: f32,
    pub min_scale: f32,
    pub max_scale: f32,
    /// Optional world-height placement bounds (meters). `None` = unconstrained.
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    /// Maximum terrain slope (degrees) this type may occupy. `None` = unconstrained.
    pub max_slope_degrees: Option<f32>,
    pub enabled: bool,
    /// Reserved for future renderer integration; does not load assets.
    pub render_key: DoodadRenderKey,
    /// Reserved for future procedural filters (e.g. "forest_edge").
    pub placement_tags: Vec<String>,
    /// Reserved for future biome-aware spawning.
    pub biome_tags: Vec<String>,
    /// Reserved relative spawn weight for procedural generation.
    pub spawn_weight: f32,
    /// Reserved reference to a future placement rule set.
    pub rule_ref: Option<String>,
}

impl DoodadDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: DoodadDefinitionId,
        kind: DoodadKind,
        display_name: impl Into<String>,
        placement_radius_meters: f32,
        min_scale: f32,
        max_scale: f32,
        min_height: Option<f32>,
        max_height: Option<f32>,
        max_slope_degrees: Option<f32>,
        enabled: bool,
        render_key: DoodadRenderKey,
    ) -> Self {
        Self {
            id,
            kind,
            display_name: display_name.into(),
            placement_radius_meters,
            min_scale,
            max_scale,
            min_height,
            max_height,
            max_slope_degrees,
            enabled,
            render_key,
            placement_tags: Vec::new(),
            biome_tags: Vec::new(),
            spawn_weight: 1.0,
            rule_ref: None,
        }
    }
}
