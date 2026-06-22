use bevy::prelude::*;

use super::definition_id::UnitDefinitionId;
use super::render_key::UnitRenderKey;

/// Authoritative description of a unit type (ADR-027 U1).
///
/// Catalog definitions are independent of world instances, ECS, and rendering.
/// `faction_tag` is **content metadata** from Excel — not runtime ownership.
/// Future instances will track dynamic affiliation via `OwnerId` / `TeamId` /
/// `AffiliationId` on runtime state (U2+), not on this definition.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct UnitDefinition {
    pub id: UnitDefinitionId,
    pub display_name: String,
    /// Excel `Faction` — design-time grouping only; not instance ownership.
    pub faction_tag: String,
    pub level: u32,
    pub base_hp: u32,
    pub strength: u32,
    pub dexterity: u32,
    pub constitution: u32,
    pub agility: u32,
    pub charisma: u32,
    pub intelligence: u32,
    pub power_rating: f32,
    pub tier: String,
    pub move_speed_mps: f32,
    pub collision_radius_meters: f32,
    pub max_slope_degrees: f32,
    pub enabled: bool,
    pub render_key: UnitRenderKey,
}

impl UnitDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: UnitDefinitionId,
        display_name: impl Into<String>,
        faction_tag: impl Into<String>,
        level: u32,
        base_hp: u32,
        strength: u32,
        dexterity: u32,
        constitution: u32,
        agility: u32,
        charisma: u32,
        intelligence: u32,
        power_rating: f32,
        tier: impl Into<String>,
        move_speed_mps: f32,
        collision_radius_meters: f32,
        max_slope_degrees: f32,
        enabled: bool,
        render_key: UnitRenderKey,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            faction_tag: faction_tag.into(),
            level,
            base_hp,
            strength,
            dexterity,
            constitution,
            agility,
            charisma,
            intelligence,
            power_rating,
            tier: tier.into(),
            move_speed_mps,
            collision_radius_meters,
            max_slope_degrees,
            enabled,
            render_key,
        }
    }
}
