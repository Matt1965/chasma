//! Animation layering architecture (A4 / D4).
//!
//! Presentation-only: lower body, upper body, and a reserved overlay slot.

use std::time::Duration;

use bevy::prelude::Reflect;

use crate::world::{
    AnimationClipKey, AnimationProfile, AttackPhase, ChunkLayout, UnitDefinition, UnitRecord,
    UnitState, WeaponDefinition,
};

use super::components::DeathPresentation;
use super::locomotion_polish::LocomotionPresentationState;
use super::settings::UnitAnimationSettings;
use super::sync_timing::is_attack_animation_phase;

/// Mask group ids used in [`AnimationGraph`] mask bitfields (A4).
pub mod mask_groups {
    /// Hips, legs, root — locomotion ownership.
    pub const LOWER_BODY: u32 = 0;
    /// Spine, arms, head — combat / tool ownership.
    pub const UPPER_BODY: u32 = 1;
    /// Reserved for future overlay clips (hit VFX, buffs, etc.) — seam retained (A4).
    #[allow(dead_code)]
    pub const OVERLAY: u32 = 2;
}

/// Clip-node mask: disable upper-body bones so only lower body plays.
pub const LOWER_BODY_CLIP_MASK: u64 = 1 << mask_groups::UPPER_BODY;
/// Clip-node mask: disable lower-body bones so only upper body plays.
pub const UPPER_BODY_CLIP_MASK: u64 = 1 << mask_groups::LOWER_BODY;
/// Full-body clip — no mask groups disabled.
pub const FULL_BODY_CLIP_MASK: u64 = 0;

/// How playback combines animation layers for a unit (A4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum UnitAnimationLayeringMode {
    /// Locomotion and attack play on separate masked branches simultaneously.
    Masked,
    /// Pre-A4 exclusive playback when skeleton/profile cannot support masks.
    #[default]
    FullBodyExclusive,
}

/// Lower-body presentation slot (A4/A5).
#[derive(Debug, Clone, PartialEq)]
pub enum LowerBodyIntent {
    Locomotion {
        clip: AnimationClipKey,
        speed: f32,
        looping: bool,
        blend: Duration,
    },
    Turn {
        clip: AnimationClipKey,
        speed: f32,
        blend: Duration,
    },
    /// Full-body override active — lower body not driven separately.
    Suppressed,
}

/// Upper-body presentation slot (A4). Only attack is active today.
#[derive(Debug, Clone, PartialEq)]
pub enum UpperBodyIntent {
    None,
    Attack {
        weapon_id: crate::world::WeaponDefinitionId,
        phase: AttackPhase,
        blend: Duration,
        blend_out: Duration,
    },
}

/// Overlay slot — framework only; no active behavior in A4 (D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayIntent {
    #[default]
    None,
}

/// Full-body overrides with highest presentation priority (A4).
#[derive(Debug, Clone, PartialEq)]
pub enum FullBodyOverride {
    None,
    Death {
        blend: Duration,
        freeze_pose: bool,
    },
    /// D3 behavior preserved until overlay layer is implemented.
    HitReaction {
        blend: Duration,
    },
}

/// Composite presentation intent across animation layers (A4).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitLayeredAnimationIntent {
    pub lower: LowerBodyIntent,
    pub upper: UpperBodyIntent,
    pub overlay: OverlayIntent,
    pub override_mode: FullBodyOverride,
}

impl UnitLayeredAnimationIntent {
    /// Whether masked dual-layer playback should be attempted.
    pub fn uses_masked_layers(&self) -> bool {
        matches!(self.override_mode, FullBodyOverride::None)
            && matches!(self.upper, UpperBodyIntent::Attack { .. })
            && matches!(
                self.lower,
                LowerBodyIntent::Locomotion { .. } | LowerBodyIntent::Turn { .. }
            )
    }
}

/// Derive layered intent from authoritative simulation state (A4).
pub fn derive_layered_animation_intent(
    record: &UnitRecord,
    definition: &UnitDefinition,
    profile: &AnimationProfile,
    weapon: &WeaponDefinition,
    settings: &UnitAnimationSettings,
    layout: ChunkLayout,
    locomotion: &mut LocomotionPresentationState,
    delta_seconds: f32,
    hit_reaction_requested: bool,
    hit_reaction_active: bool,
) -> Option<UnitLayeredAnimationIntent> {
    if !profile.enabled {
        return None;
    }

    if matches!(record.state, UnitState::Dead) {
        return Some(UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Suppressed,
            upper: UpperBodyIntent::None,
            overlay: OverlayIntent::None,
            override_mode: FullBodyOverride::Death {
                blend: Duration::from_millis(profile.death_blend_ms as u64),
                freeze_pose: profile.resolve_death_clip_name().is_none(),
            },
        });
    }

    let upper = if let Some(cycle) = &record.attack_cycle {
        if is_attack_animation_phase(cycle.phase) {
            UpperBodyIntent::Attack {
                weapon_id: weapon.id.clone(),
                phase: cycle.phase,
                blend: Duration::from_millis(weapon.attack_animation.blend_in_ms as u64),
                blend_out: Duration::from_millis(weapon.attack_animation.blend_out_ms as u64),
            }
        } else {
            UpperBodyIntent::None
        }
    } else {
        UpperBodyIntent::None
    };

    if !matches!(upper, UpperBodyIntent::Attack { .. })
        && (hit_reaction_requested || hit_reaction_active)
    {
        return Some(UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Suppressed,
            upper: UpperBodyIntent::None,
            overlay: OverlayIntent::None,
            override_mode: FullBodyOverride::HitReaction {
                blend: Duration::from_millis(profile.hit_reaction_blend_ms as u64),
            },
        });
    }

    let lower = super::locomotion_polish::resolve_polished_lower_body(
        record,
        definition,
        profile,
        settings,
        layout,
        locomotion,
        delta_seconds,
    )?;

    Some(UnitLayeredAnimationIntent {
        lower,
        upper,
        overlay: OverlayIntent::None,
        override_mode: FullBodyOverride::None,
    })
}

/// Layered intent for corpse entities after world removal (A4).
pub fn derive_layered_death_presentation_intent(
    profile: &AnimationProfile,
    presentation: &DeathPresentation,
    settings: &UnitAnimationSettings,
) -> Option<UnitLayeredAnimationIntent> {
    if !profile.enabled {
        return None;
    }
    let _ = settings;
    Some(UnitLayeredAnimationIntent {
        lower: LowerBodyIntent::Suppressed,
        upper: UpperBodyIntent::None,
        overlay: OverlayIntent::None,
        override_mode: FullBodyOverride::Death {
            blend: Duration::from_millis(profile.death_blend_ms as u64),
            freeze_pose: presentation.freeze_pose || profile.resolve_death_clip_name().is_none(),
        },
    })
}

/// Returns whether a bone path belongs to the upper-body mask group (A4).
pub fn bone_path_is_upper_body(path: &str, split_bone: &str) -> bool {
    let split = split_bone.trim();
    if split.is_empty() {
        return false;
    }
    if path == split {
        return true;
    }
    if let Some(suffix) = path.strip_prefix('/') {
        return bone_path_is_upper_body(suffix, split);
    }
    if path.ends_with(&format!("/{split}")) {
        return true;
    }
    path.split('/').any(|segment| segment == split)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        AnimationProfileId, AttackCycle, CombatState, HitMode, LocalPosition, NavigationPath,
        UnitDefinitionId, UnitPlacement, UnitRenderKey, UnitSource, UnitVitals, WeaponDefinitionId,
        WorldPosition,
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
        .with_layering(Some("Spine".to_string()))
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
        }
    }

    fn moving_state() -> UnitState {
        UnitState::Moving {
            target: WorldPosition::new(
                crate::world::ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::ONE),
            ),
            path: NavigationPath::new(vec![]),
            waypoint_index: 0,
        }
    }

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn derive_intent(
        record: &UnitRecord,
        definition: &UnitDefinition,
        hit_requested: bool,
        hit_active: bool,
    ) -> UnitLayeredAnimationIntent {
        let mut locomotion = LocomotionPresentationState::default();
        derive_layered_animation_intent(
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
    fn walk_plus_attack_layers_independently() {
        let cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        let intent = derive_intent(
            &sample_record(moving_state(), Some(cycle)),
            &sample_definition(2.9),
            false,
            false,
        );
        assert!(matches!(
            intent.lower,
            LowerBodyIntent::Locomotion {
                clip: AnimationClipKey::Walk,
                ..
            }
        ));
        assert!(matches!(intent.upper, UpperBodyIntent::Attack { .. }));
        assert!(intent.uses_masked_layers());
    }

    #[test]
    fn run_plus_attack_layers_independently() {
        let cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        let intent = derive_intent(
            &sample_record(moving_state(), Some(cycle)),
            &sample_definition(4.5),
            false,
            false,
        );
        assert!(matches!(
            intent.lower,
            LowerBodyIntent::Locomotion {
                clip: AnimationClipKey::Run,
                ..
            }
        ));
        assert!(matches!(intent.upper, UpperBodyIntent::Attack { .. }));
    }

    #[test]
    fn idle_plus_attack_layers_independently() {
        let cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        let intent = derive_intent(
            &sample_record(UnitState::Idle, Some(cycle)),
            &sample_definition(4.5),
            false,
            false,
        );
        assert!(matches!(
            intent.lower,
            LowerBodyIntent::Locomotion {
                clip: AnimationClipKey::Idle,
                ..
            }
        ));
        assert!(matches!(intent.upper, UpperBodyIntent::Attack { .. }));
    }

    #[test]
    fn attack_phase_change_does_not_change_lower_layer() {
        let mut cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        let walking = derive_intent(
            &sample_record(moving_state(), Some(cycle.clone())),
            &sample_definition(2.9),
            false,
            false,
        );
        cycle.begin_recovery(0.1);
        let recovering = derive_intent(
            &sample_record(moving_state(), Some(cycle)),
            &sample_definition(2.9),
            false,
            false,
        );
        assert_eq!(walking.lower, recovering.lower);
        assert!(matches!(
            recovering.upper,
            UpperBodyIntent::Attack {
                phase: AttackPhase::Recovery,
                ..
            }
        ));
    }

    #[test]
    fn death_overrides_all_layers() {
        let cycle = AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2);
        let intent = derive_intent(
            &sample_record(UnitState::Dead, Some(cycle)),
            &sample_definition(4.5),
            true,
            true,
        );
        assert!(matches!(
            intent.override_mode,
            FullBodyOverride::Death { .. }
        ));
        assert!(!intent.uses_masked_layers());
    }

    #[test]
    fn hit_reaction_uses_full_body_override() {
        let intent = derive_intent(
            &sample_record(UnitState::Idle, None),
            &sample_definition(4.5),
            true,
            false,
        );
        assert!(matches!(
            intent.override_mode,
            FullBodyOverride::HitReaction { .. }
        ));
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
        assert!(matches!(intent.override_mode, FullBodyOverride::None));
        assert!(matches!(intent.upper, UpperBodyIntent::Attack { .. }));
    }

    #[test]
    fn bone_path_upper_body_classification() {
        assert!(bone_path_is_upper_body(
            "Armature/Hips/Spine/Chest",
            "Spine"
        ));
        assert!(!bone_path_is_upper_body("Armature/Hips/LeftLeg", "Spine"));
    }

    #[test]
    fn worlddata_unchanged_by_layered_intent() {
        let record = sample_record(
            UnitState::Idle,
            Some(AttackCycle::start_windup(crate::world::UnitId::new(2), 0.2)),
        );
        let mut locomotion = LocomotionPresentationState::default();
        let _ = derive_layered_animation_intent(
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
