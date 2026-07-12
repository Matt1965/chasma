//! Attack playback speed / seek helpers — presentation only (A2).

use crate::world::{
    AttackCycle, AttackPhase, AttackPlaybackPolicy, WeaponDefinition, WeaponTiming,
};

/// Active attack phases that map to the weapon attack clip (A2).
pub fn is_attack_animation_phase(phase: AttackPhase) -> bool {
    matches!(
        phase,
        AttackPhase::Windup | AttackPhase::Strike | AttackPhase::Recovery
    )
}

/// Authoritative windup + recovery duration the attack clip must span (A2).
pub fn attack_cycle_playback_seconds(weapon: &WeaponDefinition) -> f32 {
    let timing = WeaponTiming::from_weapon(weapon);
    (timing.windup_seconds + timing.recovery_seconds).max(0.001)
}

/// Playback speed so a clip spans the authoritative attack window (A2).
pub fn attack_playback_speed(clip_duration_seconds: f32, weapon: &WeaponDefinition) -> f32 {
    let target = attack_cycle_playback_seconds(weapon);
    let clip = clip_duration_seconds.max(0.001);
    match weapon.attack_animation.playback_policy {
        AttackPlaybackPolicy::ScaleToCycle => (clip / target).max(0.05),
    }
}

/// Normalized clip position where simulation strike should land (A2).
pub fn weapon_normalized_strike_time(weapon: &WeaponDefinition) -> f32 {
    weapon.attack_animation.normalized_strike_time_clamped()
}

/// Whether to seek the active attack clip to the strike pose (A2).
pub fn should_seek_attack_strike(
    weapon: &WeaponDefinition,
    cycle: &AttackCycle,
    clip_duration_seconds: f32,
) -> Option<f32> {
    if cycle.phase != AttackPhase::Strike {
        return None;
    }
    let strike_time = weapon_normalized_strike_time(weapon);
    if clip_duration_seconds <= 0.0 {
        return Some(strike_time);
    }
    let timing = WeaponTiming::from_weapon(weapon);
    let speed = attack_playback_speed(clip_duration_seconds, weapon);
    let expected_elapsed = timing.windup_seconds * speed / clip_duration_seconds;
    if (expected_elapsed - strike_time).abs() > 0.05 {
        Some(strike_time)
    } else {
        None
    }
}

/// Stable key for one attack clip play — survives phase transitions within a cycle (A2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttackPlaybackKey {
    pub weapon_id: crate::world::WeaponDefinitionId,
    pub target: crate::world::UnitId,
}

pub fn attack_playback_key(cycle: &AttackCycle, weapon: &WeaponDefinition) -> AttackPlaybackKey {
    AttackPlaybackKey {
        weapon_id: weapon.id.clone(),
        target: cycle.target,
    }
}

/// Whether a new attack clip should start (A2).
pub fn should_restart_attack_playback(
    previous_phase: Option<AttackPhase>,
    cycle: &AttackCycle,
    previous_key: Option<&AttackPlaybackKey>,
    weapon: &WeaponDefinition,
) -> bool {
    let key = attack_playback_key(cycle, weapon);
    if previous_key != Some(&key) {
        return true;
    }
    matches!(
        (previous_phase, cycle.phase),
        (None, AttackPhase::Windup)
            | (Some(AttackPhase::Cooldown), AttackPhase::Windup)
            | (Some(AttackPhase::Recovery), AttackPhase::Windup)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        DamageType, HitMode, TargetFilter, WeaponAttackAnimation, WeaponDefinition,
        WeaponDefinitionId,
    };

    fn sample_weapon(windup: f32, recovery: f32) -> WeaponDefinition {
        WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_test"),
            "Test",
            "Test",
            5.0,
            DamageType::Blunt,
            1.5,
            1.0,
            windup,
            recovery,
            HitMode::Melee,
            None,
            0.0,
            "attack_test",
            vec![TargetFilter::Enemies],
            None,
            true,
        )
    }

    #[test]
    fn playback_speed_scales_clip_to_cycle() {
        let weapon = sample_weapon(0.2, 0.15);
        let speed = attack_playback_speed(1.0, &weapon);
        assert!((speed - (1.0 / 0.35)).abs() < 0.001);
    }

    #[test]
    fn strike_seek_when_drift_exceeds_tolerance() {
        let weapon = sample_weapon(0.1, 0.1).with_attack_animation(WeaponAttackAnimation {
            normalized_strike_time: 0.8,
            ..WeaponAttackAnimation::default()
        });
        let cycle = AttackCycle {
            target: crate::world::UnitId::new(2),
            phase: AttackPhase::Strike,
            phase_remaining_seconds: 0.0,
            struck_this_cycle: true,
        };
        assert_eq!(should_seek_attack_strike(&weapon, &cycle, 1.0), Some(0.8));
    }

    #[test]
    fn restart_attack_on_fresh_windup_after_cooldown() {
        let weapon = sample_weapon(0.2, 0.1);
        let cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        let key = attack_playback_key(&cycle, &weapon);
        assert!(should_restart_attack_playback(
            Some(AttackPhase::Cooldown),
            &cycle,
            Some(&key),
            &weapon
        ));
    }

    #[test]
    fn no_restart_within_same_attack_cycle() {
        let weapon = sample_weapon(0.2, 0.1);
        let cycle = AttackCycle {
            target: crate::world::UnitId::new(2),
            phase: AttackPhase::Recovery,
            phase_remaining_seconds: 0.05,
            struck_this_cycle: true,
        };
        let key = attack_playback_key(&cycle, &weapon);
        assert!(!should_restart_attack_playback(
            Some(AttackPhase::Strike),
            &cycle,
            Some(&key),
            &weapon
        ));
    }
}
