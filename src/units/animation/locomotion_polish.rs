//! Advanced locomotion presentation polish (A5 / D5).
//!
//! Presentation-only: heading-aware turns, walk/run hysteresis, speed smoothing,
//! and blend selection. Simulation and [`WorldData`] are never mutated.

use std::f32::consts::PI;
use std::time::Duration;

use bevy::prelude::{Quat, Vec2, Vec3};

use crate::world::stabilized_movement_heading;
use crate::world::{
    AnimationClipKey, AnimationProfile, ChunkLayout, UnitDefinition, UnitRecord, UnitState,
};

use super::layers::LowerBodyIntent;
use super::settings::UnitAnimationSettings;

/// Bevy/glTF default: model forward is **-Z** in local space (A5).
pub const MODEL_FORWARD_AXIS: Vec3 = Vec3::NEG_Z;

/// Per-unit locomotion polish cache — presentation only (A5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocomotionPresentationState {
    pub last_locomotion_clip: Option<AnimationClipKey>,
    pub smoothed_speed: f32,
    pub turn_remaining_seconds: Option<f32>,
    pub active_turn_clip: Option<AnimationClipKey>,
    pub was_moving: bool,
}

/// Snapshot for dev/debug readout (A5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocomotionDebugSnapshot {
    pub locomotion_clip: Option<AnimationClipKey>,
    pub playback_speed: f32,
    pub heading_delta_degrees: Option<f32>,
    pub turn_active: bool,
    pub alignment_factor: f32,
}

/// Resolve polished lower-body locomotion intent (A5).
pub fn resolve_polished_lower_body(
    record: &UnitRecord,
    definition: &UnitDefinition,
    profile: &AnimationProfile,
    settings: &UnitAnimationSettings,
    layout: ChunkLayout,
    locomotion: &mut LocomotionPresentationState,
    delta_seconds: f32,
) -> Option<LowerBodyIntent> {
    tick_turn_timer(locomotion, delta_seconds);

    if let Some(turn_clip) = active_turn_intent(locomotion, profile, settings) {
        return Some(turn_clip);
    }

    let heading_delta = movement_heading_delta(record, layout);
    if let Some(turn_clip) = try_begin_turn(record, profile, settings, locomotion, heading_delta) {
        return Some(turn_clip);
    }

    let desired =
        locomotion_clip_with_hysteresis(record, definition, profile, settings, locomotion);
    let (_clip_name, resolved) = profile.resolve_clip_name(desired)?;

    let target_speed = locomotion_playback_speed(
        record,
        definition,
        profile,
        settings,
        resolved,
        heading_delta,
    );
    locomotion.smoothed_speed = smooth_speed(
        locomotion.smoothed_speed,
        target_speed,
        settings.locomotion_speed_smoothing,
    );
    locomotion.last_locomotion_clip = Some(resolved);
    locomotion.was_moving = matches!(record.state, UnitState::Moving { .. });

    let blend = locomotion_blend_duration(record, locomotion, resolved, settings);

    Some(LowerBodyIntent::Locomotion {
        clip: resolved,
        speed: locomotion.smoothed_speed,
        looping: true,
        blend,
    })
}

fn tick_turn_timer(state: &mut LocomotionPresentationState, delta_seconds: f32) {
    let Some(remaining) = state.turn_remaining_seconds.as_mut() else {
        return;
    };
    *remaining -= delta_seconds;
    if *remaining <= 0.0 {
        state.turn_remaining_seconds = None;
        state.active_turn_clip = None;
    }
}

fn active_turn_intent(
    state: &LocomotionPresentationState,
    profile: &AnimationProfile,
    settings: &UnitAnimationSettings,
) -> Option<LowerBodyIntent> {
    let clip = state.active_turn_clip?;
    if state.turn_remaining_seconds? <= 0.0 {
        return None;
    }
    if profile.resolve_clip_name(clip).is_none() {
        return None;
    }
    Some(LowerBodyIntent::Turn {
        clip,
        speed: settings.turn_playback_speed,
        blend: Duration::from_millis(settings.turn_blend_ms),
    })
}

fn try_begin_turn(
    record: &UnitRecord,
    profile: &AnimationProfile,
    settings: &UnitAnimationSettings,
    state: &mut LocomotionPresentationState,
    heading_delta: Option<f32>,
) -> Option<LowerBodyIntent> {
    let delta = heading_delta?;
    let abs_deg = delta.to_degrees().abs();
    if abs_deg < settings.turn_in_place_degrees {
        return None;
    }

    let turn_clip = if delta > 0.0 {
        AnimationClipKey::TurnRight
    } else {
        AnimationClipKey::TurnLeft
    };
    if profile.resolve_clip_name(turn_clip).is_none() {
        return None;
    }

    let allow_turn = match &record.state {
        UnitState::Idle => abs_deg >= settings.turn_in_place_degrees,
        UnitState::Moving { .. } => abs_deg >= settings.turn_adjust_degrees,
        UnitState::Dead => false,
    };
    if !allow_turn {
        return None;
    }

    let duration = profile
        .turn_duration_seconds(turn_clip)
        .unwrap_or(settings.turn_default_seconds);
    state.active_turn_clip = Some(turn_clip);
    state.turn_remaining_seconds = Some(duration / settings.turn_playback_speed.max(0.05));

    Some(LowerBodyIntent::Turn {
        clip: turn_clip,
        speed: settings.turn_playback_speed,
        blend: Duration::from_millis(settings.turn_blend_ms),
    })
}

fn locomotion_clip_with_hysteresis(
    record: &UnitRecord,
    definition: &UnitDefinition,
    profile: &AnimationProfile,
    settings: &UnitAnimationSettings,
    state: &LocomotionPresentationState,
) -> AnimationClipKey {
    match &record.state {
        UnitState::Idle | UnitState::Dead => AnimationClipKey::Idle,
        UnitState::Moving { .. } => {
            let reference = profile.locomotion_reference_speed_mps.max(0.01);
            let speed = definition.move_speed_mps;
            let was_run = state.last_locomotion_clip == Some(AnimationClipKey::Run);
            if was_run {
                if speed >= reference * settings.run_exit_ratio {
                    AnimationClipKey::Run
                } else {
                    AnimationClipKey::Walk
                }
            } else if speed >= reference * settings.run_enter_ratio {
                AnimationClipKey::Run
            } else {
                AnimationClipKey::Walk
            }
        }
    }
}

pub fn locomotion_playback_speed(
    _record: &UnitRecord,
    definition: &UnitDefinition,
    profile: &AnimationProfile,
    settings: &UnitAnimationSettings,
    clip: AnimationClipKey,
    heading_delta: Option<f32>,
) -> f32 {
    let reference = profile.locomotion_reference_speed_mps.max(0.01);
    let base = if matches!(clip, AnimationClipKey::Idle) {
        settings.locomotion_speed_scale
    } else {
        (definition.move_speed_mps / reference).max(0.05) * settings.locomotion_speed_scale
    };
    base * heading_alignment_factor(heading_delta, settings)
}

pub fn heading_alignment_factor(
    heading_delta: Option<f32>,
    settings: &UnitAnimationSettings,
) -> f32 {
    let Some(delta) = heading_delta else {
        return 1.0;
    };
    let abs_deg = delta.to_degrees().abs();
    if abs_deg <= settings.foot_slide_min_alignment_degrees {
        return 1.0;
    }
    let t = ((abs_deg - settings.foot_slide_min_alignment_degrees)
        / (90.0 - settings.foot_slide_min_alignment_degrees))
        .clamp(0.0, 1.0);
    1.0 - t * settings.foot_slide_max_slowdown
}

fn smooth_speed(current: f32, target: f32, alpha: f32) -> f32 {
    if current <= 0.0 {
        return target;
    }
    current + (target - current) * alpha.clamp(0.0, 1.0)
}

fn locomotion_blend_duration(
    record: &UnitRecord,
    state: &LocomotionPresentationState,
    resolved: AnimationClipKey,
    settings: &UnitAnimationSettings,
) -> Duration {
    let stopping = state.was_moving && matches!(record.state, UnitState::Idle);
    if stopping {
        return Duration::from_millis(settings.stop_blend_ms);
    }
    if state
        .last_locomotion_clip
        .is_some_and(|prev| prev != resolved)
    {
        return match (state.last_locomotion_clip, resolved) {
            (Some(AnimationClipKey::Idle), AnimationClipKey::Walk)
            | (Some(AnimationClipKey::Walk), AnimationClipKey::Run) => {
                Duration::from_millis(settings.accel_blend_ms)
            }
            (Some(AnimationClipKey::Run), AnimationClipKey::Walk)
            | (Some(AnimationClipKey::Walk), AnimationClipKey::Idle) => {
                Duration::from_millis(settings.decel_blend_ms)
            }
            _ => Duration::from_millis(settings.default_blend_ms),
        };
    }
    Duration::from_millis(settings.default_blend_ms)
}

/// Signed radians from model forward to movement direction (+ = turn right).
pub fn movement_heading_delta(record: &UnitRecord, layout: ChunkLayout) -> Option<f32> {
    let direction = movement_direction_xz(record, layout)?;
    let forward = model_forward_xz(record.placement.rotation);
    signed_angle_xz(forward, direction)
}

pub fn model_forward_xz(rotation: Quat) -> Vec2 {
    let forward = rotation * MODEL_FORWARD_AXIS;
    let xz = Vec2::new(forward.x, forward.z);
    if xz.length_squared() <= 1e-8 {
        Vec2::Y
    } else {
        xz.normalize()
    }
}

pub fn movement_direction_xz(record: &UnitRecord, layout: ChunkLayout) -> Option<Vec2> {
    match &record.state {
        UnitState::Moving {
            path,
            waypoint_index,
            ..
        } => stabilized_movement_heading(record.placement.position, path, *waypoint_index, layout)
            .map(|heading| heading.direction_xz),
        UnitState::Idle | UnitState::Dead => None,
    }
}

pub fn signed_angle_xz(from: Vec2, to: Vec2) -> Option<f32> {
    if from.length_squared() <= 1e-8 || to.length_squared() <= 1e-8 {
        return None;
    }
    let from = from.normalize();
    let to = to.normalize();
    let sin = from.x * to.y - from.y * to.x;
    let cos = from.dot(to).clamp(-1.0, 1.0);
    Some(sin.atan2(cos))
}

fn normalize_angle(angle: f32) -> f32 {
    let mut a = angle % (2.0 * PI);
    if a > PI {
        a -= 2.0 * PI;
    } else if a < -PI {
        a += 2.0 * PI;
    }
    a
}

pub fn locomotion_debug_snapshot(
    record: &UnitRecord,
    layout: ChunkLayout,
    locomotion: &LocomotionPresentationState,
    settings: &UnitAnimationSettings,
) -> LocomotionDebugSnapshot {
    let heading_delta = movement_heading_delta(record, layout);
    LocomotionDebugSnapshot {
        locomotion_clip: locomotion
            .active_turn_clip
            .or(locomotion.last_locomotion_clip),
        playback_speed: locomotion.smoothed_speed,
        heading_delta_degrees: heading_delta.map(|delta| delta.to_degrees()),
        turn_active: locomotion.turn_remaining_seconds.is_some_and(|t| t > 0.0),
        alignment_factor: heading_alignment_factor(heading_delta, settings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, CombatState, LocalPosition, NavigationPath, UnitDefinitionId, UnitId,
        UnitPlacement, UnitSource, UnitVitals, WorldPosition,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn sample_profile() -> AnimationProfile {
        AnimationProfile::new(
            crate::world::AnimationProfileId::new("humanoid"),
            "Idle",
            Some("Walk".to_string()),
            Some("Run".to_string()),
            4.0,
            true,
        )
        .with_turn_clips(
            Some("TurnLeft".to_string()),
            Some("TurnRight".to_string()),
            Some(0.6),
            Some(0.6),
        )
    }

    fn sample_definition(speed: f32) -> UnitDefinition {
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
            speed,
            0.6,
            40.0,
            crate::world::WeaponDefinitionId::new("weapon_wolf_bite"),
            true,
            crate::world::UnitRenderKey::reserved("wolf"),
        )
    }

    fn sample_record(state: UnitState) -> UnitRecord {
        UnitRecord {
            id: UnitId::new(1),
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
            affiliation: Affiliation::Neutral,
            vitals: UnitVitals::full(10),
            combat_state: CombatState::Peaceful,
            attack_cycle: None,
        }
    }

    #[test]
    fn walk_run_hysteresis_prevents_flicker() {
        let settings = UnitAnimationSettings::default();
        let profile = sample_profile();
        let mut state = LocomotionPresentationState {
            last_locomotion_clip: Some(AnimationClipKey::Run),
            ..Default::default()
        };
        let record = sample_record(UnitState::Moving {
            target: WorldPosition::new(
                crate::world::ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::ONE),
            ),
            path: NavigationPath::new(vec![]),
            waypoint_index: 0,
        });
        let clip = locomotion_clip_with_hysteresis(
            &record,
            &sample_definition(3.1),
            &profile,
            &settings,
            &state,
        );
        assert_eq!(clip, AnimationClipKey::Run);
        state.last_locomotion_clip = Some(AnimationClipKey::Walk);
        let clip = locomotion_clip_with_hysteresis(
            &record,
            &sample_definition(2.9),
            &profile,
            &settings,
            &state,
        );
        assert_eq!(clip, AnimationClipKey::Walk);
    }

    #[test]
    fn playback_speed_scales_with_move_speed() {
        let settings = UnitAnimationSettings::default();
        let profile = sample_profile();
        let slow = locomotion_playback_speed(
            &sample_record(UnitState::Idle),
            &sample_definition(2.0),
            &profile,
            &settings,
            AnimationClipKey::Walk,
            None,
        );
        let fast = locomotion_playback_speed(
            &sample_record(UnitState::Idle),
            &sample_definition(8.0),
            &profile,
            &settings,
            AnimationClipKey::Run,
            None,
        );
        assert!(fast > slow);
    }

    #[test]
    fn heading_misalignment_slows_playback() {
        let settings = UnitAnimationSettings::default();
        let aligned = heading_alignment_factor(Some(0.0), &settings);
        let misaligned = heading_alignment_factor(Some(std::f32::consts::FRAC_PI_2), &settings);
        assert!(misaligned < aligned);
    }

    #[test]
    fn turn_starts_when_idle_and_heading_mismatch() {
        let mut state = LocomotionPresentationState::default();
        let mut record = sample_record(UnitState::Idle);
        record.placement.rotation = Quat::IDENTITY;
        let path = NavigationPath::new(vec![WorldPosition::new(
            crate::world::ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(0.0, 0.0, 10.0)),
        )]);
        record.state = UnitState::Moving {
            target: path.waypoints[0],
            path,
            waypoint_index: 0,
        };
        record.placement.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let intent = resolve_polished_lower_body(
            &record,
            &sample_definition(4.0),
            &sample_profile(),
            &UnitAnimationSettings::default(),
            layout(),
            &mut state,
            0.0,
        )
        .unwrap();
        assert!(matches!(
            intent,
            LowerBodyIntent::Turn {
                clip: AnimationClipKey::TurnLeft,
                ..
            }
        ));
    }

    #[test]
    fn stop_uses_longer_blend() {
        let settings = UnitAnimationSettings::default();
        let state = LocomotionPresentationState {
            was_moving: true,
            last_locomotion_clip: Some(AnimationClipKey::Walk),
            ..Default::default()
        };
        let blend = locomotion_blend_duration(
            &sample_record(UnitState::Idle),
            &state,
            AnimationClipKey::Idle,
            &settings,
        );
        assert_eq!(blend, Duration::from_millis(settings.stop_blend_ms));
    }

    #[test]
    fn worlddata_unchanged_by_polish() {
        let record = sample_record(UnitState::Idle);
        let _ = movement_heading_delta(&record, layout());
        assert!(matches!(record.state, UnitState::Idle));
    }
}
