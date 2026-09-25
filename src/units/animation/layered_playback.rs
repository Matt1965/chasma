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
    use_alternate_attack_variant: bool,
) -> LayeredPlaybackTargets {
    if !matches!(intent.override_mode, FullBodyOverride::None) {
        return LayeredPlaybackTargets {
            full_body: resolve_full_body_override(&intent.override_mode, built, profile, settings),
            ..Default::default()
        };
    }

    let use_layered_locomotion =
        mode == UnitAnimationLayeringMode::Masked && intent.uses_masked_layers();
    let lower = resolve_lower_target(&intent.lower, built, use_layered_locomotion);
    let upper = resolve_upper_target(
        &intent.upper,
        built,
        weapon,
        mode,
        use_alternate_attack_variant,
    );

    if use_layered_locomotion {
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
        FullBodyOverride::CombatIdle { weapon_id, blend } => {
            let node = built
                .combat_idle_nodes
                .get(weapon_id)
                .or(built.idle_fallback_node.as_ref())
                .copied()?;
            let duration = built
                .combat_idle_durations
                .get(weapon_id)
                .or_else(|| built.locomotion_durations.get(&AnimationClipKey::Idle))
                .copied()
                .unwrap_or(1.0);
            Some(LayerClipTarget {
                clip: AnimationPlaybackClip::CombatIdle(weapon_id.clone()),
                node,
                duration,
                speed: 1.0,
                blend: *blend,
                looping: true,
                freeze_pose: false,
            })
        }
    }
}

fn resolve_lower_target(
    intent: &LowerBodyIntent,
    built: &DefinitionAnimationGraph,
    use_layered_locomotion: bool,
) -> Option<LayerClipTarget> {
    let locomotion_nodes = if use_layered_locomotion {
        &built.layered_locomotion_nodes
    } else {
        &built.locomotion_nodes
    };
    match intent {
        LowerBodyIntent::Locomotion {
            clip,
            speed,
            looping,
            blend,
        } => {
            let node = *locomotion_nodes.get(clip)?;
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
            let node = *locomotion_nodes.get(clip)?;
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
    use_alternate_attack_variant: bool,
) -> Option<LayerClipTarget> {
    let UpperBodyIntent::Attack {
        weapon_id, blend, ..
    } = intent
    else {
        return None;
    };
    let weapon = weapon?;
    let variant_node = use_alternate_attack_variant
        && built.attack_variant_nodes.contains_key(weapon_id);
    let node = if mode == UnitAnimationLayeringMode::Masked {
        if variant_node {
            *built.attack_variant_nodes.get(weapon_id)?
        } else {
            *built.attack_nodes.get(weapon_id)?
        }
    } else {
        if variant_node {
            built
                .attack_variant_nodes
                .get(weapon_id)
                .or(built.idle_fallback_node.as_ref())
                .copied()?
        } else {
            built
                .attack_nodes
                .get(weapon_id)
                .or(built.idle_fallback_node.as_ref())
                .copied()?
        }
    };
    let _ = resolve_attack_clip_name(weapon, use_alternate_attack_variant);
    let duration = if variant_node {
        built
            .attack_variant_durations
            .get(weapon_id)
            .copied()
    } else {
        built.attack_durations.get(weapon_id).copied()
    }
    .or_else(|| built.locomotion_durations.get(&AnimationClipKey::Idle).copied())
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
    use crate::world::{AnimationProfileId, DamageType, HitMode, TargetFilter, WeaponDefinitionId};

    fn empty_built() -> DefinitionAnimationGraph {
        let share_key = crate::units::animation::AnimationGraphShareKey {
            profile_id: AnimationProfileId::new("humanoid"),
            gltf_asset_path: "units/test.glb".to_string(),
        };
        DefinitionAnimationGraph {
            graph: Handle::default(),
            locomotion_nodes: Default::default(),
            layered_locomotion_nodes: Default::default(),
            attack_nodes: Default::default(),
            attack_variant_nodes: Default::default(),
            combat_idle_nodes: Default::default(),
            locomotion_durations: Default::default(),
            attack_durations: Default::default(),
            attack_variant_durations: Default::default(),
            combat_idle_durations: Default::default(),
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

    fn sample_weapon(weapon_id: &str, animation_key: &str) -> crate::world::WeaponDefinition {
        crate::world::WeaponDefinition::new(
            WeaponDefinitionId::new(weapon_id),
            "T",
            "T",
            5.0,
            DamageType::Blunt,
            1.5,
            1.0,
            0.2,
            0.1,
            HitMode::Melee,
            None,
            0.0,
            animation_key,
            vec![TargetFilter::Enemies],
            None,
            true,
        )
    }

    fn sample_profile() -> AnimationProfile {
        AnimationProfile::new(
            AnimationProfileId::new("humanoid"),
            "Idle",
            None,
            None,
            4.0,
            true,
        )
    }

    struct TestGraphNodes {
        full_body: AnimationNodeIndex,
        layered: AnimationNodeIndex,
        attack: AnimationNodeIndex,
    }

    fn built_with_locomotion_and_attack(
        clip: AnimationClipKey,
        weapon: &crate::world::WeaponDefinition,
    ) -> (DefinitionAnimationGraph, TestGraphNodes) {
        let mut built = empty_built();
        let nodes = TestGraphNodes {
            full_body: AnimationNodeIndex::new(1),
            layered: AnimationNodeIndex::new(2),
            attack: AnimationNodeIndex::new(3),
        };
        built.locomotion_nodes.insert(clip, nodes.full_body);
        built.layered_locomotion_nodes.insert(clip, nodes.layered);
        built.locomotion_durations.insert(clip, 1.0);
        built
            .attack_nodes
            .insert(weapon.id.clone(), nodes.attack);
        built.attack_durations.insert(weapon.id.clone(), 0.8);
        (built, nodes)
    }

    fn locomotion_intent(clip: AnimationClipKey) -> UnitLayeredAnimationIntent {
        UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Locomotion {
                clip,
                speed: 1.0,
                looping: true,
                blend: Duration::ZERO,
            },
            upper: UpperBodyIntent::None,
            overlay: OverlayIntent::None,
            override_mode: FullBodyOverride::None,
        }
    }

    fn attack_intent(
        clip: AnimationClipKey,
        weapon: &crate::world::WeaponDefinition,
    ) -> UnitLayeredAnimationIntent {
        UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Locomotion {
                clip,
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
        }
    }

    fn resolve(
        intent: &UnitLayeredAnimationIntent,
        mode: UnitAnimationLayeringMode,
        built: &DefinitionAnimationGraph,
        weapon: Option<&crate::world::WeaponDefinition>,
    ) -> LayeredPlaybackTargets {
        resolve_layered_playback_targets(
            intent,
            mode,
            built,
            weapon,
            &sample_profile(),
            &UnitAnimationSettings::default(),
            false,
        )
    }

    #[test]
    fn ordinary_locomotion_uses_full_body_nodes() {
        for clip in [AnimationClipKey::Idle, AnimationClipKey::Walk, AnimationClipKey::Run] {
            let weapon = sample_weapon("weapon_fists", "Punch_Jab");
            let (built, nodes) = built_with_locomotion_and_attack(clip, &weapon);
            let targets = resolve(
                &locomotion_intent(clip),
                UnitAnimationLayeringMode::Masked,
                &built,
                None,
            );
            assert_eq!(targets.full_body.as_ref().map(|t| t.node), Some(nodes.full_body));
            assert!(targets.lower.is_none());
            assert!(targets.upper.is_none());
        }
    }

    #[test]
    fn masked_idle_attack_uses_layered_locomotion_and_upper_attack() {
        let weapon = sample_weapon("weapon_fists", "Punch_Jab");
        let (built, nodes) = built_with_locomotion_and_attack(AnimationClipKey::Idle, &weapon);
        let targets = resolve(
            &attack_intent(AnimationClipKey::Idle, &weapon),
            UnitAnimationLayeringMode::Masked,
            &built,
            Some(&weapon),
        );
        assert_eq!(targets.lower.as_ref().map(|t| t.node), Some(nodes.layered));
        assert_eq!(targets.upper.as_ref().map(|t| t.node), Some(nodes.attack));
        assert_ne!(targets.lower.as_ref().map(|t| t.node), Some(nodes.full_body));
        assert!(targets.full_body.is_none());
    }

    #[test]
    fn masked_walk_attack_uses_layered_locomotion_and_upper_attack() {
        let weapon = sample_weapon("weapon_fists", "Punch_Jab");
        let (built, nodes) = built_with_locomotion_and_attack(AnimationClipKey::Walk, &weapon);
        let targets = resolve(
            &attack_intent(AnimationClipKey::Walk, &weapon),
            UnitAnimationLayeringMode::Masked,
            &built,
            Some(&weapon),
        );
        assert_eq!(targets.lower.as_ref().map(|t| t.node), Some(nodes.layered));
        assert_eq!(targets.upper.as_ref().map(|t| t.node), Some(nodes.attack));
        assert_ne!(targets.lower.as_ref().map(|t| t.node), Some(nodes.full_body));
        assert!(targets.full_body.is_none());
    }

    #[test]
    fn masked_run_attack_uses_layered_locomotion_and_upper_attack() {
        let weapon = sample_weapon("weapon_sword", "Sword_Attack");
        let (built, nodes) = built_with_locomotion_and_attack(AnimationClipKey::Run, &weapon);
        let targets = resolve(
            &attack_intent(AnimationClipKey::Run, &weapon),
            UnitAnimationLayeringMode::Masked,
            &built,
            Some(&weapon),
        );
        assert_eq!(targets.lower.as_ref().map(|t| t.node), Some(nodes.layered));
        assert_eq!(targets.upper.as_ref().map(|t| t.node), Some(nodes.attack));
        assert_ne!(targets.lower.as_ref().map(|t| t.node), Some(nodes.full_body));
        assert!(targets.full_body.is_none());
    }

    #[test]
    fn full_body_exclusive_attack_does_not_dual_play() {
        let weapon = sample_weapon("weapon_fists", "Punch_Jab");
        let (built, nodes) = built_with_locomotion_and_attack(AnimationClipKey::Walk, &weapon);
        let targets = resolve(
            &attack_intent(AnimationClipKey::Walk, &weapon),
            UnitAnimationLayeringMode::FullBodyExclusive,
            &built,
            Some(&weapon),
        );
        assert_eq!(targets.full_body.as_ref().map(|t| t.node), Some(nodes.attack));
        assert!(targets.lower.is_none());
        assert!(targets.upper.is_none());
    }

    #[test]
    fn death_remains_full_body_override() {
        let weapon = sample_weapon("weapon_fists", "Punch_Jab");
        let (mut built, _) = built_with_locomotion_and_attack(AnimationClipKey::Idle, &weapon);
        let death_node = AnimationNodeIndex::new(9);
        built.death_node = Some(death_node);
        built.death_duration = Some(2.0);
        let intent = UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Suppressed,
            upper: UpperBodyIntent::None,
            overlay: OverlayIntent::None,
            override_mode: FullBodyOverride::Death {
                blend: Duration::ZERO,
                freeze_pose: false,
            },
        };
        let targets = resolve(&intent, UnitAnimationLayeringMode::Masked, &built, Some(&weapon));
        assert_eq!(targets.full_body.as_ref().map(|t| t.node), Some(death_node));
        assert!(targets.lower.is_none());
        assert!(targets.upper.is_none());
    }

    #[test]
    fn hit_reaction_remains_full_body_override() {
        let weapon = sample_weapon("weapon_fists", "Punch_Jab");
        let (mut built, _) = built_with_locomotion_and_attack(AnimationClipKey::Idle, &weapon);
        let hit_node = AnimationNodeIndex::new(10);
        built.hit_reaction_node = Some(hit_node);
        built.hit_reaction_duration = Some(0.5);
        let intent = UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Locomotion {
                clip: AnimationClipKey::Idle,
                speed: 1.0,
                looping: true,
                blend: Duration::ZERO,
            },
            upper: UpperBodyIntent::None,
            overlay: OverlayIntent::None,
            override_mode: FullBodyOverride::HitReaction {
                blend: Duration::ZERO,
            },
        };
        let targets = resolve(&intent, UnitAnimationLayeringMode::Masked, &built, Some(&weapon));
        assert_eq!(targets.full_body.as_ref().map(|t| t.node), Some(hit_node));
        assert!(targets.lower.is_none());
        assert!(targets.upper.is_none());
    }

    #[test]
    fn missing_upper_clip_continues_lower_in_masked_mode() {
        let weapon = sample_weapon("weapon_test", "missing_attack");
        let mut built = empty_built();
        let full_body_walk = AnimationNodeIndex::new(1);
        let layered_walk = AnimationNodeIndex::new(2);
        built
            .locomotion_nodes
            .insert(AnimationClipKey::Walk, full_body_walk);
        built
            .layered_locomotion_nodes
            .insert(AnimationClipKey::Walk, layered_walk);
        built
            .locomotion_durations
            .insert(AnimationClipKey::Walk, 1.0);
        let intent = attack_intent(AnimationClipKey::Walk, &weapon);
        let targets = resolve(
            &intent,
            UnitAnimationLayeringMode::Masked,
            &built,
            Some(&weapon),
        );
        assert_eq!(targets.lower.as_ref().map(|t| t.node), Some(layered_walk));
        assert_ne!(targets.lower.as_ref().map(|t| t.node), Some(full_body_walk));
        assert!(targets.upper.is_none());
        assert!(targets.full_body.is_none());
    }
}
