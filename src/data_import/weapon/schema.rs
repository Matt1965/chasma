//! Excel column schema and conversion into [`WeaponDefinition`].

use crate::world::{
    AttackPlaybackPolicy, DamageType, HitMode, TargetFilter, WeaponAttackAnimation,
    WeaponDefinition, WeaponDefinitionId,
};

/// Required worksheet column headers from the workbook `Weapons` sheet.
pub const REQUIRED_COLUMNS: &[&str] = &[
    "Weapon ID",
    "Name",
    "Description",
    "Damage",
    "Damage Type",
    "Range",
    "Attacks Per Second",
    "Windup",
    "Recovery",
    "Hit Mode",
    "Projectile Key",
    "Animation Key",
    "Target Filters",
    "Stat Scaling",
    "Enabled",
];

/// Optional worksheet columns (A2 weapon animation).
#[allow(dead_code)]
pub const OPTIONAL_COLUMNS: &[&str] = &[
    "Projectile Speed",
    "Attack Playback Policy",
    "Normalized Strike Time",
    "Blend In",
    "Blend Out",
    "Attack Variant",
];

pub const DEFAULT_NORMALIZED_STRIKE_TIME: f32 = 0.42;
pub const DEFAULT_ATTACK_BLEND_MS: u32 = 150;

/// Raw row parsed from the `Weapons` sheet before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponImportRow {
    pub row_number: usize,
    pub weapon_id: String,
    pub name: String,
    pub description: String,
    pub damage: f32,
    pub damage_type: DamageType,
    pub range_meters: f32,
    pub attacks_per_second: f32,
    pub windup_seconds: f32,
    pub recovery_seconds: f32,
    pub hit_mode: HitMode,
    pub projectile_key: Option<String>,
    pub projectile_speed_mps: f32,
    pub animation_key: String,
    pub attack_playback_policy: AttackPlaybackPolicy,
    pub normalized_strike_time: f32,
    pub attack_blend_in_ms: u32,
    pub attack_blend_out_ms: u32,
    pub attack_variant: Option<String>,
    pub target_filters: Vec<TargetFilter>,
    pub stat_scaling: Option<String>,
    pub enabled: bool,
    pub enabled_was_blank: bool,
}

impl WeaponImportRow {
    pub fn to_definition(&self) -> WeaponDefinition {
        WeaponDefinition::new(
            WeaponDefinitionId::new(self.weapon_id.trim()),
            self.name.trim(),
            self.description.trim(),
            self.damage,
            self.damage_type,
            self.range_meters,
            self.attacks_per_second,
            self.windup_seconds,
            self.recovery_seconds,
            self.hit_mode,
            self.projectile_key.clone(),
            self.projectile_speed_mps,
            self.animation_key.trim(),
            self.target_filters.clone(),
            self.stat_scaling.clone(),
            self.enabled,
        )
        .with_attack_animation(WeaponAttackAnimation {
            playback_policy: self.attack_playback_policy,
            normalized_strike_time: self.normalized_strike_time,
            blend_in_ms: self.attack_blend_in_ms,
            blend_out_ms: self.attack_blend_out_ms,
            variant: self.attack_variant.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_type_parsing() {
        assert_eq!(DamageType::parse("Physical").unwrap(), DamageType::Physical);
        assert_eq!(DamageType::parse("fire").unwrap(), DamageType::Fire);
        assert!(DamageType::parse("unknown").is_err());
    }

    #[test]
    fn hit_mode_parsing() {
        assert_eq!(HitMode::parse("Melee").unwrap(), HitMode::Melee);
        assert_eq!(
            HitMode::parse("RangedInstant").unwrap(),
            HitMode::RangedInstant
        );
        assert!(HitMode::parse("Laser").is_err());
    }

    #[test]
    fn target_filter_parsing() {
        let filters = TargetFilter::parse_list("Enemies, Wildlife").unwrap();
        assert_eq!(filters, vec![TargetFilter::Enemies, TargetFilter::Wildlife]);
        assert_eq!(
            TargetFilter::parse_list("").unwrap(),
            vec![TargetFilter::Enemies]
        );
    }
}
