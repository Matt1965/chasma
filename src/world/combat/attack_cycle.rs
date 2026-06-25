//! Weapon timing derivation from catalog definitions (ADR-058 C5).

use bevy::prelude::warn;

use crate::world::WeaponDefinition;

/// Derived timing from [`WeaponDefinition`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponTiming {
    pub attack_period_seconds: f32,
    pub windup_seconds: f32,
    pub recovery_seconds: f32,
    pub cooldown_seconds: f32,
    pub timing_overflow: bool,
}

impl WeaponTiming {
    pub fn from_weapon(weapon: &WeaponDefinition) -> Self {
        let attack_period_seconds = if weapon.attacks_per_second <= 0.0 {
            f32::INFINITY
        } else {
            1.0 / weapon.attacks_per_second
        };
        let windup_seconds = weapon.windup_seconds.max(0.0);
        let recovery_seconds = weapon.recovery_seconds.max(0.0);
        let raw_cooldown = attack_period_seconds - windup_seconds - recovery_seconds;
        let timing_overflow = attack_period_seconds.is_finite()
            && windup_seconds + recovery_seconds > attack_period_seconds;
        if timing_overflow {
            warn!(
                "weapon {} windup+recovery exceeds attack period; cycle uses windup+recovery",
                weapon.id.0
            );
        }
        Self {
            attack_period_seconds,
            windup_seconds,
            recovery_seconds,
            cooldown_seconds: raw_cooldown.max(0.0),
            timing_overflow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        DamageType, HitMode, TargetFilter, WeaponDefinition, WeaponDefinitionId,
    };

    fn test_weapon(aps: f32, windup: f32, recovery: f32) -> WeaponDefinition {
        WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_test"),
            "Test",
            "Test",
            5.0,
            DamageType::Blunt,
            1.5,
            aps,
            windup,
            recovery,
            HitMode::Melee,
            None,
            "attack_test",
            vec![TargetFilter::Enemies],
            None,
            true,
        )
    }

    #[test]
    fn cooldown_remainder_clamps_at_zero() {
        let timing = WeaponTiming::from_weapon(&test_weapon(2.0, 0.4, 0.3));
        assert!((timing.attack_period_seconds - 0.5).abs() < f32::EPSILON);
        assert!((timing.cooldown_seconds - 0.0).abs() < f32::EPSILON);
        assert!(timing.timing_overflow);
    }

    #[test]
    fn attacks_per_second_controls_period() {
        let timing = WeaponTiming::from_weapon(&test_weapon(1.0, 0.1, 0.1));
        assert!((timing.attack_period_seconds - 1.0).abs() < f32::EPSILON);
        assert!((timing.cooldown_seconds - 0.8).abs() < f32::EPSILON);
        assert!(!timing.timing_overflow);
    }
}
