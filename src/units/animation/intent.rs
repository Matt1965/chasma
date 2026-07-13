use std::time::Duration;

use crate::world::{
    AnimationClipKey, AnimationProfile, AttackPhase, UnitDefinition, UnitRecord, WeaponDefinition,
};

use super::layers::{
    FullBodyOverride, LowerBodyIntent, UnitLayeredAnimationIntent, UpperBodyIntent,
    derive_layered_animation_intent, derive_layered_death_presentation_intent,
};
use super::settings::UnitAnimationSettings;

/// Derived presentation intent from authoritative simulation state (A1/A2).
#[derive(Debug, Clone, PartialEq)]
pub enum UnitAnimationIntent {
    Locomotion {
        clip: AnimationClipKey,
        speed: f32,
        looping: bool,
        blend: Duration,
    },
    Attack {
        weapon_id: crate::world::WeaponDefinitionId,
        phase: AttackPhase,
        blend: Duration,
        blend_out: Duration,
    },
    Death {
        blend: Duration,
        freeze_pose: bool,
    },
    HitReaction {
        blend: Duration,
    },
}

impl UnitAnimationIntent {
    pub fn looping(&self) -> bool {
        match self {
            Self::Locomotion { looping, .. } => *looping,
            Self::Attack { .. } | Self::Death { .. } | Self::HitReaction { .. } => false,
        }
    }
}

/// Presentation intent for corpse entities after world removal (A3).
pub(crate) fn derive_death_presentation_intent(
    profile: &AnimationProfile,
    presentation: &super::components::DeathPresentation,
    settings: &UnitAnimationSettings,
) -> Option<UnitAnimationIntent> {
    let layered = derive_layered_death_presentation_intent(profile, presentation, settings)?;
    Some(flatten_layered_intent(&layered))
}

/// Pure mapping: authoritative record + catalogs → presentation intent.
///
/// Returns `None` when the unit has no animation profile (static model).
pub fn derive_unit_animation_intent(
    record: &UnitRecord,
    definition: &UnitDefinition,
    profile: &AnimationProfile,
    weapon: &WeaponDefinition,
    settings: &UnitAnimationSettings,
    layout: crate::world::ChunkLayout,
    locomotion: &mut super::locomotion_polish::LocomotionPresentationState,
    delta_seconds: f32,
    hit_reaction_requested: bool,
    hit_reaction_active: bool,
) -> Option<UnitAnimationIntent> {
    let layered = derive_layered_animation_intent(
        record,
        definition,
        profile,
        weapon,
        settings,
        layout,
        locomotion,
        delta_seconds,
        hit_reaction_requested,
        hit_reaction_active,
    )?;
    Some(flatten_layered_intent(&layered))
}

fn flatten_layered_intent(layered: &UnitLayeredAnimationIntent) -> UnitAnimationIntent {
    match &layered.override_mode {
        FullBodyOverride::Death { blend, freeze_pose } => UnitAnimationIntent::Death {
            blend: *blend,
            freeze_pose: *freeze_pose,
        },
        FullBodyOverride::HitReaction { blend } => {
            UnitAnimationIntent::HitReaction { blend: *blend }
        }
        FullBodyOverride::None => match &layered.upper {
            UpperBodyIntent::Attack {
                weapon_id,
                phase,
                blend,
                blend_out,
            } => UnitAnimationIntent::Attack {
                weapon_id: weapon_id.clone(),
                phase: *phase,
                blend: *blend,
                blend_out: *blend_out,
            },
            UpperBodyIntent::None => match &layered.lower {
                LowerBodyIntent::Locomotion {
                    clip,
                    speed,
                    looping,
                    blend,
                } => UnitAnimationIntent::Locomotion {
                    clip: *clip,
                    speed: *speed,
                    looping: *looping,
                    blend: *blend,
                },
                LowerBodyIntent::Turn { clip, speed, blend } => UnitAnimationIntent::Locomotion {
                    clip: *clip,
                    speed: *speed,
                    looping: false,
                    blend: *blend,
                },
                LowerBodyIntent::Suppressed => UnitAnimationIntent::Locomotion {
                    clip: AnimationClipKey::Idle,
                    speed: 1.0,
                    looping: true,
                    blend: Duration::from_millis(150),
                },
            },
        },
    }
}

/// Resolve attack clip name with Idle fallback (A2).
pub fn resolve_attack_clip_name(weapon: &WeaponDefinition) -> Option<&str> {
    let key = weapon.animation_key.trim();
    if key.is_empty() { None } else { Some(key) }
}

/// Playback speed for attack intent once clip duration is known (A2).
pub fn attack_intent_speed(weapon: &WeaponDefinition, clip_duration_seconds: f32) -> f32 {
    super::sync_timing::attack_playback_speed(clip_duration_seconds, weapon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::animation::LocomotionPresentationState;
    use crate::units::animation::layers::OverlayIntent;
    use crate::world::{
        AnimationProfileId, AttackCycle, CombatState, HitMode, LocalPosition, NavigationPath,
        UnitDefinitionId, UnitPlacement, UnitRenderKey, UnitSource, UnitState, UnitVitals,
        WeaponDefinitionId, WeaponTiming, WorldPosition,
    };
    use bevy::prelude::{Quat, Vec3};

    fn sample_weapon() -> WeaponDefinition {
        WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_wolf_bite"),
            "Bite",
            "Bite",
            8.0,
            crate::world::DamageType::Slashing,
            1.5,
            1.2,
            0.2,
            0.15,
            HitMode::Melee,
            None,
            0.0,
            "attack_bite",
            vec![crate::world::TargetFilter::Enemies],
            None,
            true,
        )
    }

    fn sample_record(state: UnitState, attack_cycle: Option<AttackCycle>) -> UnitRecord {
        UnitRecord {
            id: crate::world::UnitId::new(1),
            definition_id: UnitDefinitionId::new("wolf"),
            placement: UnitPlacement::new(
                WorldPosition::new(
                    crate::world::ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::ZERO),
                ),
                Quat::IDENTITY,
            ),
            state,
            source: UnitSource::Authored,
            metadata: Default::default(),
            owner_id: None,
            team_id: None,
            affiliation: crate::world::Affiliation::Neutral,
            vitals: UnitVitals::full(10),
            combat_state: CombatState::Attacking {
                target: crate::world::UnitId::new(2),
            },
            attack_cycle,
            current_space_id: Default::default(),
        }
    }

    fn sample_definition(move_speed_mps: f32) -> UnitDefinition {
        UnitDefinition::new(
            UnitDefinitionId::new("wolf"),
            "Wolf",
            "Wild",
            2,
            5,
            5,
            4,
            6,
            3,
            7,
            2,
            3,
            26.5,
            "Elite",
            move_speed_mps,
            0.6,
            40.0,
            WeaponDefinitionId::new("weapon_wolf_bite"),
            true,
            UnitRenderKey::reserved("wolf"),
        )
    }

    fn sample_profile() -> AnimationProfile {
        AnimationProfile::new(
            AnimationProfileId::new("humanoid"),
            "Idle",
            Some("Walk".to_string()),
            Some("Run".to_string()),
            4.0,
            true,
        )
        .with_presentation_clips(Some("Death".to_string()), Some("Hit".to_string()))
    }

    fn moving_state() -> UnitState {
        UnitState::Moving {
            target: WorldPosition::new(
                crate::world::ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::ONE),
            ),
            path: NavigationPath::default(),
            waypoint_index: 0,
        }
    }

    fn layout() -> crate::world::ChunkLayout {
        crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn derive_intent(
        record: &UnitRecord,
        definition: &UnitDefinition,
        hit_requested: bool,
        hit_active: bool,
    ) -> UnitAnimationIntent {
        let mut locomotion = LocomotionPresentationState::default();
        derive_unit_animation_intent(
            record,
            definition,
            &sample_profile(),
            &sample_weapon(),
            &UnitAnimationSettings::default(),
            layout(),
            &mut locomotion,
            0.016,
            hit_requested,
            hit_active,
        )
        .unwrap()
    }

    #[test]
    fn idle_when_not_moving() {
        let intent = derive_intent(
            &sample_record(UnitState::Idle, None),
            &sample_definition(4.5),
            false,
            false,
        );
        assert!(matches!(
            intent,
            UnitAnimationIntent::Locomotion {
                clip: AnimationClipKey::Idle,
                ..
            }
        ));
    }

    #[test]
    fn walk_when_moving_below_run_threshold() {
        let intent = derive_intent(
            &sample_record(moving_state(), None),
            &sample_definition(2.9),
            false,
            false,
        );
        assert!(matches!(
            intent,
            UnitAnimationIntent::Locomotion {
                clip: AnimationClipKey::Walk,
                ..
            }
        ));
    }

    #[test]
    fn melee_attack_intent_during_windup() {
        let cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        let intent = derive_intent(
            &sample_record(UnitState::Idle, Some(cycle)),
            &sample_definition(4.5),
            false,
            false,
        );
        assert!(matches!(
            intent,
            UnitAnimationIntent::Attack {
                phase: AttackPhase::Windup,
                ..
            }
        ));
    }

    #[test]
    fn ranged_attack_intent_during_recovery() {
        let mut weapon = sample_weapon();
        weapon.hit_mode = HitMode::Projectile;
        weapon.projectile_key = Some("arrow".to_string());
        let mut cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        cycle.begin_recovery(0.15);
        let mut locomotion = LocomotionPresentationState::default();
        let intent = derive_unit_animation_intent(
            &sample_record(UnitState::Idle, Some(cycle)),
            &sample_definition(4.5),
            &sample_profile(),
            &weapon,
            &UnitAnimationSettings::default(),
            layout(),
            &mut locomotion,
            0.016,
            false,
            false,
        )
        .unwrap();
        assert!(matches!(
            intent,
            UnitAnimationIntent::Attack {
                phase: AttackPhase::Recovery,
                ..
            }
        ));
    }

    #[test]
    fn move_order_returns_locomotion_during_cooldown() {
        let mut cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        cycle.begin_cooldown(0.5);
        let intent = derive_intent(
            &sample_record(moving_state(), Some(cycle)),
            &sample_definition(2.9),
            false,
            false,
        );
        assert!(matches!(
            intent,
            UnitAnimationIntent::Locomotion {
                clip: AnimationClipKey::Walk,
                ..
            }
        ));
    }

    #[test]
    fn dead_unit_uses_death_intent() {
        let intent = derive_intent(
            &sample_record(UnitState::Dead, None),
            &sample_definition(4.5),
            false,
            false,
        );
        assert!(matches!(intent, UnitAnimationIntent::Death { .. }));
    }

    #[test]
    fn missing_death_clip_freezes_pose() {
        let profile = AnimationProfile::new(
            AnimationProfileId::new("humanoid"),
            "Idle",
            Some("Walk".to_string()),
            Some("Run".to_string()),
            4.0,
            true,
        );
        let intent = flatten_layered_intent(&UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Suppressed,
            upper: UpperBodyIntent::None,
            overlay: OverlayIntent::None,
            override_mode: FullBodyOverride::Death {
                blend: Duration::from_millis(profile.death_blend_ms as u64),
                freeze_pose: profile.resolve_death_clip_name().is_none(),
            },
        });
        assert!(matches!(
            intent,
            UnitAnimationIntent::Death {
                freeze_pose: true,
                ..
            }
        ));
    }

    #[test]
    fn death_overrides_hit_reaction() {
        let intent = derive_intent(
            &sample_record(UnitState::Dead, None),
            &sample_definition(4.5),
            true,
            true,
        );
        assert!(matches!(intent, UnitAnimationIntent::Death { .. }));
    }

    #[test]
    fn hit_reaction_when_damaged_and_not_attacking() {
        let intent = derive_intent(
            &sample_record(UnitState::Idle, None),
            &sample_definition(4.5),
            true,
            false,
        );
        assert!(matches!(intent, UnitAnimationIntent::HitReaction { .. }));
    }

    #[test]
    fn attack_overrides_hit_reaction() {
        let cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        let intent = derive_intent(
            &sample_record(UnitState::Idle, Some(cycle)),
            &sample_definition(4.5),
            true,
            false,
        );
        assert!(matches!(intent, UnitAnimationIntent::Attack { .. }));
    }

    #[test]
    fn missing_attack_clip_name_is_none() {
        let mut weapon = sample_weapon();
        weapon.animation_key = "   ".to_string();
        assert!(resolve_attack_clip_name(&weapon).is_none());
    }

    #[test]
    fn attack_speed_scales_to_weapon_timing() {
        let weapon = sample_weapon();
        let speed = attack_intent_speed(&weapon, 1.0);
        let timing = WeaponTiming::from_weapon(&weapon);
        let expected = 1.0 / (timing.windup_seconds + timing.recovery_seconds);
        assert!((speed - expected).abs() < 0.001);
    }

    #[test]
    fn cancelled_attack_returns_locomotion() {
        let intent = derive_intent(
            &sample_record(moving_state(), None),
            &sample_definition(2.9),
            false,
            false,
        );
        assert!(matches!(intent, UnitAnimationIntent::Locomotion { .. }));
    }

    #[test]
    fn worlddata_types_unchanged_by_intent_derivation() {
        let record = sample_record(
            UnitState::Idle,
            Some(AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2)),
        );
        let mut locomotion = LocomotionPresentationState::default();
        let _ = derive_unit_animation_intent(
            &record,
            &sample_definition(4.5),
            &sample_profile(),
            &sample_weapon(),
            &UnitAnimationSettings::default(),
            layout(),
            &mut locomotion,
            0.016,
            false,
            false,
        );
        assert!(record.attack_cycle.is_some());
    }
}
