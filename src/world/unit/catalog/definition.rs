use bevy::prelude::*;

use super::definition_id::UnitDefinitionId;
use super::render_key::UnitRenderKey;
use crate::world::InventoryProfileId;
use crate::world::unit::animation_profile::AnimationProfileId;
use crate::world::weapon::WeaponDefinitionId;

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
    /// Combat max HP copied to instances at spawn (ADR-055 C2).
    pub max_hp: u32,
    /// Reserved for future stamina system — no behavior in C2.
    pub stamina_max: Option<u32>,
    /// Reserved for future energy system — no behavior in C2.
    pub energy_max: Option<u32>,
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
    /// Uniform glTF scene scale at spawn (1.0 = mesh units as authored).
    pub render_scale: f32,
    pub default_weapon_id: WeaponDefinitionId,
    pub enabled: bool,
    pub render_key: UnitRenderKey,
    /// Optional locomotion animation profile (A1). None = static model.
    pub animation_profile_id: Option<AnimationProfileId>,
    /// Worker capability flags (ADR-085 B8).
    pub work_capabilities: super::work::UnitWorkCapabilities,
    /// Optional inventory container profile (ADR-087 I1). None = no inventory.
    pub inventory_profile_id: Option<InventoryProfileId>,
    /// Authoritative corpse lifetime override in simulation ticks (ADR-089 I3).
    pub corpse_lifetime_ticks: Option<u64>,
}

impl UnitDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: UnitDefinitionId,
        display_name: impl Into<String>,
        faction_tag: impl Into<String>,
        level: u32,
        base_hp: u32,
        max_hp: u32,
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
        default_weapon_id: WeaponDefinitionId,
        enabled: bool,
        render_key: UnitRenderKey,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            faction_tag: faction_tag.into(),
            level,
            base_hp,
            max_hp,
            stamina_max: None,
            energy_max: None,
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
            render_scale: 1.0,
            default_weapon_id,
            enabled,
            render_key,
            animation_profile_id: None,
            work_capabilities: super::work::UnitWorkCapabilities::default(),
            inventory_profile_id: None,
            corpse_lifetime_ticks: None,
        }
    }

    pub fn with_corpse_lifetime_ticks(mut self, ticks: u64) -> Self {
        self.corpse_lifetime_ticks = Some(ticks);
        self
    }

    pub fn with_inventory_profile_id(mut self, profile_id: InventoryProfileId) -> Self {
        self.inventory_profile_id = Some(profile_id);
        self
    }

    pub fn with_work_capabilities(
        mut self,
        capabilities: super::work::UnitWorkCapabilities,
    ) -> Self {
        self.work_capabilities = capabilities;
        self
    }
}
