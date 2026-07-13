//! Presentation-only animation LOD (A6 / D6).
//!
//! Distance-based tiers throttle intent derivation — never simulation.

use bevy::prelude::*;

use crate::world::{CombatState, UnitId, UnitRecord};

/// Animation update tier (presentation only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum AnimationLod {
    #[default]
    Full,
    Reduced,
    Frozen,
}

impl AnimationLod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Reduced => "Reduced",
            Self::Frozen => "Frozen",
        }
    }
}

/// Tunable animation LOD thresholds (A6).
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct AnimationLodSettings {
    pub enabled: bool,
    /// XZ distance from camera focus at or below this → Full.
    pub full_distance_meters: f32,
    /// Beyond full_distance + margin until this → Reduced.
    pub reduced_distance_meters: f32,
    /// Beyond reduced_distance + margin → Frozen.
    pub frozen_distance_meters: f32,
    pub hysteresis_margin_meters: f32,
    /// Minimum seconds between intent re-derivation at Reduced LOD.
    pub reduced_update_interval_seconds: f32,
    /// Selected units stay Full when enabled.
    pub promote_selected: bool,
    /// Inspector focus unit stays Full when enabled.
    pub promote_inspected: bool,
    /// Attacking units within this distance of focus stay Full.
    pub combat_full_distance_meters: f32,
}

impl Default for AnimationLodSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            full_distance_meters: 80.0,
            reduced_distance_meters: 160.0,
            frozen_distance_meters: 280.0,
            hysteresis_margin_meters: 12.0,
            reduced_update_interval_seconds: 0.25,
            promote_selected: true,
            promote_inspected: true,
            combat_full_distance_meters: 120.0,
        }
    }
}

/// Per-unit LOD cache — survives render recreation (A6).
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationLodPresentationState {
    pub lod: AnimationLod,
    pub distance_meters: f32,
    pub next_intent_eval_at: f32,
    pub player_frozen: bool,
}

impl Default for AnimationLodPresentationState {
    fn default() -> Self {
        Self {
            lod: AnimationLod::Full,
            distance_meters: 0.0,
            next_intent_eval_at: 0.0,
            player_frozen: false,
        }
    }
}

/// Client-local focus overrides for animation LOD (A6).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationPresentationFocus {
    pub inspected_unit: Option<crate::world::UnitId>,
}
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct AnimationPresentationMetrics {
    pub animated_units: u32,
    pub full_count: u32,
    pub reduced_count: u32,
    pub frozen_count: u32,
    pub intent_evaluations: u32,
    pub transitions_applied: u32,
    pub shared_graph_count: u32,
    pub definition_graph_count: u32,
}

impl AnimationPresentationMetrics {
    pub fn reset_frame(&mut self) {
        *self = Self::default();
    }
}

/// XZ distance from camera focus to a world position.
pub fn animation_distance_meters(camera_focus: Vec3, unit_position: Vec3) -> f32 {
    let delta = unit_position - camera_focus;
    Vec2::new(delta.x, delta.z).length()
}

/// Resolve desired LOD from distance and promotion rules (no hysteresis).
pub fn raw_animation_lod(
    distance_meters: f32,
    settings: &AnimationLodSettings,
    unit_id: Option<UnitId>,
    record: Option<&UnitRecord>,
    selected: &std::collections::HashSet<UnitId>,
    inspected: Option<UnitId>,
) -> AnimationLod {
    if !settings.enabled {
        return AnimationLod::Full;
    }

    if let Some(id) = unit_id {
        if settings.promote_selected && selected.contains(&id) {
            return AnimationLod::Full;
        }
        if settings.promote_inspected && inspected == Some(id) {
            return AnimationLod::Full;
        }
    }

    if let (Some(record), Some(_)) = (record, unit_id) {
        if matches!(record.combat_state, CombatState::Attacking { .. })
            && distance_meters <= settings.combat_full_distance_meters
        {
            return AnimationLod::Full;
        }
    }

    if distance_meters <= settings.full_distance_meters {
        AnimationLod::Full
    } else if distance_meters <= settings.reduced_distance_meters {
        AnimationLod::Reduced
    } else if distance_meters <= settings.frozen_distance_meters {
        AnimationLod::Frozen
    } else {
        AnimationLod::Frozen
    }
}

/// Apply hysteresis when changing tiers to avoid threshold flicker (A6).
pub fn resolve_animation_lod(
    distance_meters: f32,
    previous: AnimationLod,
    settings: &AnimationLodSettings,
    unit_id: Option<UnitId>,
    record: Option<&UnitRecord>,
    selected: &std::collections::HashSet<UnitId>,
    inspected: Option<UnitId>,
) -> AnimationLod {
    let raw = raw_animation_lod(
        distance_meters,
        settings,
        unit_id,
        record,
        selected,
        inspected,
    );
    if !settings.enabled {
        return AnimationLod::Full;
    }

    let margin = settings.hysteresis_margin_meters;
    match (previous, raw) {
        (AnimationLod::Full, AnimationLod::Reduced)
            if distance_meters <= settings.full_distance_meters + margin =>
        {
            AnimationLod::Full
        }
        (AnimationLod::Reduced, AnimationLod::Full)
            if distance_meters >= settings.full_distance_meters - margin =>
        {
            AnimationLod::Reduced
        }
        (AnimationLod::Reduced, AnimationLod::Frozen)
            if distance_meters <= settings.reduced_distance_meters + margin =>
        {
            AnimationLod::Reduced
        }
        (AnimationLod::Frozen, AnimationLod::Reduced)
            if distance_meters >= settings.reduced_distance_meters - margin =>
        {
            AnimationLod::Frozen
        }
        (AnimationLod::Frozen, AnimationLod::Full)
            if distance_meters >= settings.frozen_distance_meters - margin =>
        {
            AnimationLod::Frozen
        }
        (_, desired) => desired,
    }
}

/// Whether presentation should re-derive intent this frame.
pub fn should_evaluate_animation_intent(
    lod: AnimationLod,
    elapsed_seconds: f32,
    next_eval_at: f32,
    force: bool,
) -> bool {
    if force {
        return true;
    }
    match lod {
        AnimationLod::Full => true,
        AnimationLod::Reduced => elapsed_seconds >= next_eval_at,
        AnimationLod::Frozen => false,
    }
}

pub fn next_reduced_eval_time(elapsed_seconds: f32, interval_seconds: f32) -> f32 {
    elapsed_seconds + interval_seconds.max(0.05)
}

pub fn lod_promoted_to_full(previous: AnimationLod, current: AnimationLod) -> bool {
    previous != AnimationLod::Full && current == AnimationLod::Full
}

pub fn lod_left_frozen(previous: AnimationLod, current: AnimationLod) -> bool {
    previous == AnimationLod::Frozen && current != AnimationLod::Frozen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, CombatState, LocalPosition, UnitDefinitionId, UnitPlacement, UnitSource,
        UnitVitals, WorldPosition,
    };
    use bevy::prelude::{Quat, Vec3};

    fn sample_record(combat: CombatState) -> UnitRecord {
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
            state: crate::world::UnitState::Idle,
            source: UnitSource::Authored,
            metadata: Default::default(),
            owner_id: None,
            team_id: None,
            affiliation: Affiliation::Neutral,
            vitals: UnitVitals::full(10),
            combat_state: combat,
            attack_cycle: None,
            current_space_id: Default::default(),
        }
    }

    #[test]
    fn selected_unit_remains_full_at_distance() {
        let settings = AnimationLodSettings::default();
        let mut selected = std::collections::HashSet::new();
        selected.insert(UnitId::new(5));
        let lod = raw_animation_lod(
            500.0,
            &settings,
            Some(UnitId::new(5)),
            None,
            &selected,
            None,
        );
        assert_eq!(lod, AnimationLod::Full);
    }

    #[test]
    fn distant_unit_gets_frozen() {
        let settings = AnimationLodSettings::default();
        let lod = raw_animation_lod(400.0, &settings, None, None, &Default::default(), None);
        assert_eq!(lod, AnimationLod::Frozen);
    }

    #[test]
    fn nearby_unit_gets_full() {
        let settings = AnimationLodSettings::default();
        let lod = raw_animation_lod(30.0, &settings, None, None, &Default::default(), None);
        assert_eq!(lod, AnimationLod::Full);
    }

    #[test]
    fn hysteresis_prevents_threshold_thrash() {
        let settings = AnimationLodSettings::default();
        let dist = settings.full_distance_meters + settings.hysteresis_margin_meters * 0.5;
        let lod = resolve_animation_lod(
            dist,
            AnimationLod::Full,
            &settings,
            None,
            None,
            &Default::default(),
            None,
        );
        assert_eq!(lod, AnimationLod::Full);
    }

    #[test]
    fn reduced_throttles_intent_eval() {
        assert!(!should_evaluate_animation_intent(
            AnimationLod::Reduced,
            1.0,
            5.0,
            false
        ));
        assert!(should_evaluate_animation_intent(
            AnimationLod::Reduced,
            6.0,
            5.0,
            false
        ));
    }

    #[test]
    fn frozen_skips_intent_eval() {
        assert!(!should_evaluate_animation_intent(
            AnimationLod::Frozen,
            100.0,
            0.0,
            false
        ));
    }

    #[test]
    fn promotion_detects_full_upgrade() {
        assert!(lod_promoted_to_full(
            AnimationLod::Frozen,
            AnimationLod::Full
        ));
        assert!(!lod_promoted_to_full(
            AnimationLod::Full,
            AnimationLod::Full
        ));
    }

    #[test]
    fn combat_near_camera_stays_full() {
        let settings = AnimationLodSettings::default();
        let record = sample_record(CombatState::Attacking {
            target: UnitId::new(2),
        });
        let lod = raw_animation_lod(
            50.0,
            &settings,
            Some(UnitId::new(1)),
            Some(&record),
            &Default::default(),
            None,
        );
        assert_eq!(lod, AnimationLod::Full);
    }

    #[test]
    fn worlddata_unchanged_by_lod_resolution() {
        let record = sample_record(CombatState::Peaceful);
        let _ = raw_animation_lod(
            200.0,
            &AnimationLodSettings::default(),
            Some(record.id),
            Some(&record),
            &Default::default(),
            None,
        );
        assert!(matches!(record.combat_state, CombatState::Peaceful));
    }
}
