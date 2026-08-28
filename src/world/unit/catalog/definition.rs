use bevy::prelude::*;

use super::definition_id::UnitDefinitionId;
use super::render_key::UnitRenderKey;
use crate::world::InventoryProfileId;
use crate::world::asset_sizing::AssetSizingDefinition;
use crate::world::perception::DEFAULT_SIGHT_RANGE_METERS;
use crate::world::relationship::{FactionId, SpeciesId};
use crate::world::unit::animation_profile::AnimationProfileId;
use crate::world::weapon::WeaponDefinitionId;

/// Default presentation yaw rate when the Units sheet omits `Turn Speed Deg/s` (UNIT-TURN-1).
pub const DEFAULT_TURN_SPEED_DEGREES_PER_SECOND: f32 = 540.0;

/// Authoritative description of a unit type (ADR-027 U1).
///
/// Catalog definitions are independent of world instances, ECS, and rendering.
/// `faction_tag` holds the faction **display name** resolved from the Factions catalog at
/// import — not runtime ownership or relationship truth.
/// Authoritative relationship identity lives in [`faction_id`] and [`species_id`].
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct UnitDefinition {
    pub id: UnitDefinitionId,
    pub display_name: String,
    /// Faction display label resolved from the Factions catalog at import.
    pub faction_tag: String,
    /// Stable faction relationship identity (ADR-132 Phase 1).
    pub faction_id: FactionId,
    /// Shared biological species identity (ADR-132 Phase 1).
    pub species_id: SpeciesId,
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
    /// Perception acquisition radius in meters (ADR-132 Phase 4).
    pub sight_range_meters: f32,
    /// Maximum visual body yaw rate toward authoritative facing (deg/s). Does not limit movement.
    pub turn_speed_degrees_per_second: f32,
    /// Uniform glTF scene scale at spawn (resolved baseline; legacy fallback when sizing unset).
    pub render_scale: f32,
    /// Metric asset sizing metadata and calculated baseline scale (ADR-097 DT1).
    pub asset_sizing: AssetSizingDefinition,
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
        faction_id: FactionId,
        species_id: SpeciesId,
        faction_display_name: impl Into<String>,
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
            faction_tag: faction_display_name.into(),
            faction_id,
            species_id,
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
            sight_range_meters: DEFAULT_SIGHT_RANGE_METERS,
            turn_speed_degrees_per_second: DEFAULT_TURN_SPEED_DEGREES_PER_SECOND,
            render_scale: 1.0,
            asset_sizing: AssetSizingDefinition::default(),
            default_weapon_id,
            enabled,
            render_key,
            animation_profile_id: None,
            work_capabilities: super::work::UnitWorkCapabilities::default(),
            inventory_profile_id: None,
            corpse_lifetime_ticks: None,
        }
    }

    /// Test/dev helper preserving the pre-Phase-1 constructor shape (faction display label only).
    #[cfg(any(test, feature = "dev"))]
    #[allow(clippy::too_many_arguments)]
    pub fn new_test(
        id: UnitDefinitionId,
        display_name: impl Into<String>,
        faction_display_name: impl Into<String>,
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
        let faction_display_name = faction_display_name.into();
        let (faction_id, species_id) = test_identity_for_faction_display(&faction_display_name);
        Self::new(
            id,
            display_name,
            faction_id,
            species_id,
            faction_display_name,
            level,
            base_hp,
            max_hp,
            strength,
            dexterity,
            constitution,
            agility,
            charisma,
            intelligence,
            power_rating,
            tier,
            move_speed_mps,
            collision_radius_meters,
            max_slope_degrees,
            default_weapon_id,
            enabled,
            render_key,
        )
    }

    pub fn with_sight_range_meters(mut self, sight_range_meters: f32) -> Self {
        self.sight_range_meters = sight_range_meters;
        self
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

#[cfg(any(test, feature = "dev"))]
fn test_identity_for_faction_display(faction_display: &str) -> (FactionId, SpeciesId) {
    match faction_display {
        "Player" => (FactionId::new("player"), SpeciesId::new("robot")),
        "Wild" => (FactionId::new("wild"), SpeciesId::new("wolf")),
        "Bandits" => (FactionId::new("bandits"), SpeciesId::new("human")),
        "Test" | "test" => (FactionId::new("wild"), SpeciesId::new("wolf")),
        other => (
            FactionId::new(other.to_ascii_lowercase()),
            SpeciesId::new("wolf"),
        ),
    }
}
