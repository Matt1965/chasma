use bevy::prelude::*;

use super::animation::WeaponAttackAnimation;
use super::definition_id::WeaponDefinitionId;

/// Damage classification for future resistance rules (ADR-054 C1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum DamageType {
    #[default]
    Physical,
    Piercing,
    Blunt,
    Slashing,
    Fire,
    Acid,
    Energy,
    True,
}

impl DamageType {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "physical" => Ok(Self::Physical),
            "piercing" => Ok(Self::Piercing),
            "blunt" => Ok(Self::Blunt),
            "slashing" => Ok(Self::Slashing),
            "fire" => Ok(Self::Fire),
            "acid" => Ok(Self::Acid),
            "energy" => Ok(Self::Energy),
            "true" => Ok(Self::True),
            other => Err(format!("unknown Damage Type `{other}`")),
        }
    }
}

/// How a weapon applies its strike (ADR-054 C1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum HitMode {
    #[default]
    Melee,
    RangedInstant,
    Projectile,
}

impl HitMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "melee" => Ok(Self::Melee),
            "rangedinstant" | "ranged instant" | "ranged_instant" => Ok(Self::RangedInstant),
            "projectile" => Ok(Self::Projectile),
            other => Err(format!("unknown Hit Mode `{other}`")),
        }
    }
}

/// Target categories a weapon may strike (ADR-054 C1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TargetFilter {
    Enemies,
    Wildlife,
    Neutral,
    Structures,
    All,
}

impl TargetFilter {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "enemies" | "enemy" => Ok(Self::Enemies),
            "wildlife" => Ok(Self::Wildlife),
            "neutral" => Ok(Self::Neutral),
            "structures" | "structure" => Ok(Self::Structures),
            "all" => Ok(Self::All),
            other => Err(format!("unknown Target Filter `{other}`")),
        }
    }

    pub fn parse_list(value: &str) -> Result<Vec<Self>, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(vec![Self::Enemies]);
        }
        trimmed.split(',').map(|part| Self::parse(part)).collect()
    }
}

/// Authoritative weapon attack description (ADR-054 C1).
///
/// Damage, range, timing, projectile, and animation keys live here — not on
/// [`crate::world::UnitDefinition`].
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct WeaponDefinition {
    pub id: WeaponDefinitionId,
    pub display_name: String,
    pub description: String,
    pub damage: f32,
    pub damage_type: DamageType,
    /// Edge-to-edge interpretation reserved for C4 combat behavior.
    pub range_meters: f32,
    pub attacks_per_second: f32,
    pub windup_seconds: f32,
    pub recovery_seconds: f32,
    pub hit_mode: HitMode,
    pub projectile_key: Option<String>,
    /// Travel speed for [`HitMode::Projectile`] weapons (m/s). Ignored for other hit modes.
    pub projectile_speed_mps: f32,
    /// glTF clip name for this weapon's attack animation (A2).
    pub animation_key: String,
    pub attack_animation: WeaponAttackAnimation,
    pub target_filters: Vec<TargetFilter>,
    /// Reserved for future stat scaling — ignored in C1 behavior.
    pub stat_scaling: Option<String>,
    pub enabled: bool,
}

impl WeaponDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: WeaponDefinitionId,
        display_name: impl Into<String>,
        description: impl Into<String>,
        damage: f32,
        damage_type: DamageType,
        range_meters: f32,
        attacks_per_second: f32,
        windup_seconds: f32,
        recovery_seconds: f32,
        hit_mode: HitMode,
        projectile_key: Option<String>,
        projectile_speed_mps: f32,
        animation_key: impl Into<String>,
        target_filters: Vec<TargetFilter>,
        stat_scaling: Option<String>,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            description: description.into(),
            damage,
            damage_type,
            range_meters,
            attacks_per_second,
            windup_seconds,
            recovery_seconds,
            hit_mode,
            projectile_key,
            projectile_speed_mps,
            animation_key: animation_key.into(),
            attack_animation: WeaponAttackAnimation::default(),
            target_filters,
            stat_scaling,
            enabled,
        }
    }

    /// Derived cooldown from workbook Attacks Per Second.
    pub fn attack_cooldown_seconds(&self) -> f32 {
        if self.attacks_per_second <= 0.0 {
            f32::INFINITY
        } else {
            1.0 / self.attacks_per_second
        }
    }

    /// Override weapon-owned attack animation parameters (A2).
    pub fn with_attack_animation(mut self, attack_animation: WeaponAttackAnimation) -> Self {
        self.attack_animation = attack_animation;
        self
    }
}
