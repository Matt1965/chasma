//! Presentation-only animation components (A1/A2).

use std::time::Duration;

use bevy::prelude::*;

use crate::world::{AnimationClipKey, AnimationProfileId, AttackPhase, UnitId, WeaponDefinitionId};

use super::locomotion_polish::LocomotionPresentationState;
use super::lod::AnimationLodPresentationState;
use super::sync_timing::AttackPlaybackKey;

/// Cached descendant [`AnimationPlayer`] for a unit render root (A1).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitAnimationPlayerLink {
    pub player_entity: Entity,
}

/// Marker: scene spawned, player discovery pending (A1).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct PendingAnimationLink;

/// Active upper-body attack weight fade (blend-in/out) (A1).
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct UpperAttackWeightFade {
    pub node: AnimationNodeIndex,
    pub remaining_seconds: f32,
    pub duration_seconds: f32,
    pub fading_in: bool,
}

/// Logical clip currently playing on the render entity (A1/A2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub enum AnimationPlaybackClip {
    Locomotion(AnimationClipKey),
    Attack(WeaponDefinitionId),
    Death,
    HitReaction,
}

/// Corpse presentation after authoritative unit removal (A3).
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct DeathPresentation {
    pub definition_id: crate::world::UnitDefinitionId,
    pub profile_id: AnimationProfileId,
    pub remaining_seconds: f32,
    /// Hold final pose when no death clip is available (A3).
    pub freeze_pose: bool,
}

/// One-frame marker: presentation-only damage feedback (A3).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct HitReactionRequested;

/// Active hit-reaction playback timer (A3).
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct HitReactionActive {
    pub remaining_seconds: f32,
}

/// Last applied clips on the render entity (A1/A2/A4).
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct UnitAnimationRuntime {
    pub current_clip: AnimationPlaybackClip,
    #[reflect(ignore)]
    pub layers: LayeredPlaybackState,
}

/// Graph and transitions installed on the player entity (A1).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitAnimationGraphInstalled;

/// Play current intent on next playback pass (scene/link/graph (re)created) (A1).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct AnimationPlaybackPending;

/// Profile used when the current graph was built (A1).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct AnimationProfileHandle {
    pub profile_id: AnimationProfileId,
}

/// Layering mode resolved for this render entity (A4).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitAnimationLayering {
    pub mode: super::layers::UnitAnimationLayeringMode,
}

impl UnitAnimationLayering {
    pub fn full_body_exclusive() -> Self {
        Self {
            mode: super::layers::UnitAnimationLayeringMode::FullBodyExclusive,
        }
    }
}

/// Active clips per presentation layer (A4/A5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayeredPlaybackState {
    pub lower: Option<AnimationPlaybackClip>,
    pub upper: Option<AnimationPlaybackClip>,
    pub full_body: Option<AnimationPlaybackClip>,
    pub lower_node: Option<AnimationNodeIndex>,
    pub upper_node: Option<AnimationNodeIndex>,
    pub full_body_node: Option<AnimationNodeIndex>,
    pub lower_speed: Option<f32>,
    pub lower_blend_ms: Option<u64>,
}

/// Persisted presentation state keyed by [`UnitId`] — survives render entity recreation (A1/A2/A4/A5).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitAnimationPersistedState {
    pub clip: AnimationPlaybackClip,
    pub layers: LayeredPlaybackState,
    pub profile_id: AnimationProfileId,
    pub last_attack_phase: Option<AttackPhase>,
    pub attack_key: Option<AttackPlaybackKey>,
    pub attack_blend_out: Option<Duration>,
    pub locomotion: LocomotionPresentationState,
    pub lod: AnimationLodPresentationState,
}

/// Index of animation presentation state per authoritative unit id (A1).
#[derive(Resource, Default, Debug)]
pub struct UnitAnimationStateIndex {
    pub states: std::collections::HashMap<UnitId, UnitAnimationPersistedState>,
}

impl UnitAnimationStateIndex {
    pub fn remove(&mut self, unit_id: UnitId) {
        self.states.remove(&unit_id);
    }
}
