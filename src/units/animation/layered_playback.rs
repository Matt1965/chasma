//! Layer target resolution and playback helpers (A4).

use std::time::Duration;

use bevy::prelude::*;

use crate::world::{AnimationClipKey, AnimationProfile, WeaponDefinition};

use super::assets::DefinitionAnimationGraph;
use super::components::{AnimationPlaybackClip, LayeredPlaybackState};
use super::intent::{attack_intent_speed, resolve_attack_clip_name};
use super::layers::{
    FullBodyOverride, LowerBodyIntent, UnitAnimationLayeringMode, UnitLayeredAnimationIntent,
    UpperBodyIntent,
};
use super::settings::UnitAnimationSettings;

/// Resolved playback for one graph node (A4).
#[derive(Debug, Clone, PartialEq)]
pub struct LayerClipTarget {
    pub clip: AnimationPlaybackClip,
    pub node: AnimationNodeIndex,
    pub duration: f32,
    pub speed: f32,
    pub blend: Duration,
    pub looping: bool,
    pub freeze_pose: bool,
}

/// Resolved targets across animation layers (A4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayeredPlaybackTargets {
    pub lower: Option<LayerClipTarget>,
    pub upper: Option<LayerClipTarget>,
    pub full_body: Option<LayerClipTarget>,
}

pub fn resolve_layered_playback_targets(
    intent: &UnitLayeredAnimationIntent,
    mode: UnitAnimationLayeringMode,
    built: &DefinitionAnimationGraph,
    weapon: Option<&WeaponDefinition>,
    profile: &AnimationProfile,
    settings: &UnitAnimationSettings,
) -> LayeredPlaybackTargets {
    if !matches!(intent.override_mode, FullBodyOverride::None) {
        return LayeredPlaybackTargets {
            full_body: resolve_full_body_override(&intent.override_mode, built, profile, settings),
            ..Default::default()
        };
    }

    let lower = resolve_lower_target(&intent.lower, built);
    let upper = resolve_upper_target(&intent.upper, built, weapon, mode);

    if mode == UnitAnimationLayeringMode::Masked && intent.uses_masked_layers() {
        return LayeredPlaybackTargets {
            lower,
            upper,
            full_body: None,
        };
    }

    // Full-body exclusive: upper body wins over lower when both are active.
    if upper.is_some() {
        LayeredPlaybackTargets {
            lower: None,
            upper: None,
            full_body: upper,
        }
    } else {
        LayeredPlaybackTargets {
            lower: None,
            upper: None,
            full_body: lower,
        }
    }
}

fn resolve_full_body_override(
    override_mode: &FullBodyOverride,
    built: &DefinitionAnimationGraph,
    _profile: &AnimationProfile,
    settings: &UnitAnimationSettings,
) -> Option<LayerClipTarget> {
    match override_mode {
        FullBodyOverride::None => None,
        FullBodyOverride::Death { blend, freeze_pose } => {
            if *freeze_pose {
                let node = built.idle_fallback_node?;
                let duration = built
                    .locomotion_durations
                    .get(&AnimationClipKey::Idle)
                    .copied()
                    .unwrap_or(1.0);
                return Some(LayerClipTarget {
                    clip: AnimationPlaybackClip::Death,
                    node,
                    duration,
                    speed: 1.0,
                    blend: *blend,
                    looping: false,
                    freeze_pose: true,
                });
            }
            let node = built.death_node.or(built.idle_fallback_node)?;
            let duration = built
                .death_duration
                .unwrap_or(settings.death_clip_hold_seconds);
            Some(LayerClipTarget {
                clip: AnimationPlaybackClip::Death,
                node,
                duration,
                speed: 1.0,
                blend: *blend,
                looping: false,
                freeze_pose: false,
            })
        }
        FullBodyOverride::HitReaction { blend } => {
            let node = built.hit_reaction_node.or(built.idle_fallback_node)?;
            let duration = built
                .hit_reaction_duration
                .unwrap_or(settings.hit_reaction_hold_seconds);
            Some(LayerClipTarget {
                clip: AnimationPlaybackClip::HitReaction,
                node,
                duration,
                speed: 1.0,
                blend: *blend,
                looping: false,
                freeze_pose: false,
            })
        }
    }
}

fn resolve_lower_target(
    intent: &LowerBodyIntent,
    built: &DefinitionAnimationGraph,
) -> Option<LayerClipTarget> {
    match intent {
        LowerBodyIntent::Locomotion {
            clip,
            speed,
            looping,
            blend,
        } => {
            let node = *built.locomotion_nodes.get(clip)?;
            let duration = built.locomotion_durations.get(clip).copied().unwrap_or(1.0);
            Some(LayerClipTarget {
                clip: AnimationPlaybackClip::Locomotion(*clip),
                node,
                duration,
                speed: *speed,
                blend: *blend,
                looping: *looping,
                freeze_pose: false,
            })
        }
        LowerBodyIntent::Turn { clip, speed, blend } => {
            let node = *built.locomotion_nodes.get(clip)?;
            let duration = built.locomotion_durations.get(clip).copied().unwrap_or(0.6);
            Some(LayerClipTarget {
                clip: AnimationPlaybackClip::Locomotion(*clip),
                node,
                duration,
                speed: *speed,
                blend: *blend,
                looping: false,
                freeze_pose: false,
            })
        }
        LowerBodyIntent::Suppressed => None,
    }
}

fn resolve_upper_target(
    intent: &UpperBodyIntent,
    built: &DefinitionAnimationGraph,
    weapon: Option<&WeaponDefinition>,
    mode: UnitAnimationLayeringMode,
) -> Option<LayerClipTarget> {
    let UpperBodyIntent::Attack {
        weapon_id, blend, ..
    } = intent
    else {
        return None;
    };
    let weapon = weapon?;
    let node = if mode == UnitAnimationLayeringMode::Masked {
        *built.attack_nodes.get(weapon_id)?
    } else {
        built
            .attack_nodes
            .get(weapon_id)
            .or(built.idle_fallback_node.as_ref())
            .copied()?
    };
    let _ = resolve_attack_clip_name(weapon);
    let duration = built
        .attack_durations
        .get(weapon_id)
        .copied()
        .or_else(|| {
            built
                .locomotion_durations
                .get(&AnimationClipKey::Idle)
                .copied()
        })
        .unwrap_or(1.0);
    let speed = attack_intent_speed(weapon, duration);
    Some(LayerClipTarget {
        clip: AnimationPlaybackClip::Attack(weapon_id.clone()),
        node,
        duration,
        speed,
        blend: *blend,
        looping: false,
        freeze_pose: false,
    })
}

pub fn layered_state_from_targets(targets: &LayeredPlaybackTargets) -> LayeredPlaybackState {
    LayeredPlaybackState {
        lower: targets.lower.as_ref().map(|t| t.clip.clone()),
        upper: targets.upper.as_ref().map(|t| t.clip.clone()),
        full_body: targets.full_body.as_ref().map(|t| t.clip.clone()),
        lower_node: targets.lower.as_ref().map(|t| t.node),
        upper_node: targets.upper.as_ref().map(|t| t.node),
        full_body_node: targets.full_body.as_ref().map(|t| t.node),
        lower_speed: targets
            .lower
            .as_ref()
            .or(targets.full_body.as_ref())
            .map(|t| t.speed),
        lower_blend_ms: targets.lower.as_ref().map(|t| t.blend.as_millis() as u64),
    }
}

pub fn primary_playback_clip(targets: &LayeredPlaybackTargets) -> AnimationPlaybackClip {
    if let Some(full) = &targets.full_body {
        return full.clip.clone();
    }
    if let Some(upper) = &targets.upper {
        return upper.clip.clone();
    }
    targets
        .lower
        .as_ref()
        .map(|t| t.clip.clone())
        .unwrap_or(AnimationPlaybackClip::Locomotion(AnimationClipKey::Idle))
}

pub fn should_restart_layered_playback(
    persisted: Option<&LayeredPlaybackState>,
    targets: &LayeredPlaybackTargets,
) -> bool {
    let Some(persisted) = persisted else {
        return true;
    };
    persisted.lower != targets.lower.as_ref().map(|t| t.clip.clone())
        || persisted.upper != targets.upper.as_ref().map(|t| t.clip.clone())
        || persisted.full_body != targets.full_body.as_ref().map(|t| t.clip.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::animation::layers::{
        OverlayIntent, UnitAnimationLayeringMode, UnitLayeredAnimationIntent,
    };
    use crate::world::{AnimationProfileId, WeaponDefinitionId};

    fn empty_built() -> DefinitionAnimationGraph {
        let share_key = crate::units::animation::AnimationGraphShareKey {
            profile_id: AnimationProfileId::new("humanoid"),
            gltf_asset_path: "units/test.glb".to_string(),
            default_weapon_id: WeaponDefinitionId::new("weapon_test"),
        };
        DefinitionAnimationGraph {
            graph: Handle::default(),
            locomotion_nodes: Default::default(),
            attack_nodes: Default::default(),
            locomotion_durations: Default::default(),
            attack_durations: Default::default(),
            death_node: None,
            death_duration: None,
            hit_reaction_node: None,
            hit_reaction_duration: None,
            idle_fallback_node: None,
            blend_root: AnimationNodeIndex::new(0),
            profile_id: AnimationProfileId::new("humanoid"),
            share_key,
        }
    }

    #[test]
    fn missing_upper_clip_continues_lower_in_masked_mode() {
        let weapon = crate::world::WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_test"),
            "T",
            "T",
            5.0,
            crate::world::DamageType::Blunt,
            1.5,
            1.0,
            0.2,
            0.1,
            crate::world::HitMode::Melee,
            None,
            0.0,
            "missing_attack",
            vec![crate::world::TargetFilter::Enemies],
            None,
            true,
        );
        let mut built = empty_built();
        let walk_node = AnimationNodeIndex::new(2);
        built
            .locomotion_nodes
            .insert(AnimationClipKey::Walk, walk_node);
        built
            .locomotion_durations
            .insert(AnimationClipKey::Walk, 1.0);
        let intent = UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Locomotion {
                clip: AnimationClipKey::Walk,
                speed: 1.0,
                looping: true,
                blend: Duration::ZERO,
            },
            upper: UpperBodyIntent::Attack {
                weapon_id: weapon.id.clone(),
                phase: crate::world::AttackPhase::Windup,
                blend: Duration::ZERO,
                blend_out: Duration::ZERO,
            },
            overlay: OverlayIntent::None,
            override_mode: FullBodyOverride::None,
        };
        let targets = resolve_layered_playback_targets(
            &intent,
            UnitAnimationLayeringMode::Masked,
            &built,
            Some(&weapon),
            &AnimationProfile::new(
                AnimationProfileId::new("humanoid"),
                "Idle",
                None,
                None,
                4.0,
                true,
            ),
            &UnitAnimationSettings::default(),
        );
        assert_eq!(targets.lower.as_ref().map(|t| t.node), Some(walk_node));
        assert!(targets.upper.is_none());
        assert!(targets.full_body.is_none());
    }
}
