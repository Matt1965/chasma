//! Optional direction smoothing for movement feel (ADR-037 U12).

use bevy::prelude::Vec2;
use std::collections::HashMap;

use crate::world::UnitId;

use super::MovementFeelSettings;

/// Per-unit smoothed heading cache (first step bypasses smoothing).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MovementSmoothingState {
    last_direction: HashMap<UnitId, Vec2>,
}

impl MovementSmoothingState {
    pub fn clear_unit(&mut self, unit_id: UnitId) {
        self.last_direction.remove(&unit_id);
    }

    pub fn clear_all(&mut self) {
        self.last_direction.clear();
    }

    /// Blend toward the new direction; first tick returns `raw_direction` unchanged.
    pub fn smooth_direction(
        &mut self,
        unit_id: UnitId,
        raw_direction: Vec2,
        settings: &MovementFeelSettings,
    ) -> Vec2 {
        if raw_direction.length_squared() <= 1e-8 {
            return raw_direction;
        }
        let raw = raw_direction.normalize();

        let Some(previous) = self.last_direction.get(&unit_id).copied() else {
            self.last_direction.insert(unit_id, raw);
            return raw;
        };

        let blended = previous * settings.direction_smooth_factor
            + raw * (1.0 - settings.direction_smooth_factor);
        if blended.length_squared() <= 1e-8 {
            self.last_direction.insert(unit_id, raw);
            return raw;
        }
        let blended = blended.normalize();

        let clamped = clamp_turn_angle(previous, blended, settings.max_smoothed_turn_radians);
        self.last_direction.insert(unit_id, clamped);
        clamped
    }

    /// Smoothing must not delay waypoint progression (ADR-037).
    pub fn should_skip_for_waypoint(
        effective_index: usize,
        path_len: usize,
        distance_to_waypoint: f32,
        step_distance: f32,
    ) -> bool {
        effective_index + 1 >= path_len || distance_to_waypoint <= step_distance * 2.0
    }
}

/// Whether direction smoothing should be bypassed so waypoint progression is unaffected.
pub fn should_skip_direction_smoothing(
    effective_index: usize,
    path_len: usize,
    distance_to_waypoint: f32,
    step_distance: f32,
) -> bool {
    MovementSmoothingState::should_skip_for_waypoint(
        effective_index,
        path_len,
        distance_to_waypoint,
        step_distance,
    )
}

fn clamp_turn_angle(from: Vec2, to: Vec2, max_radians: f32) -> Vec2 {
    if from.length_squared() <= 1e-8 {
        return to;
    }
    let from = from.normalize();
    let to = to.normalize();
    let dot = from.dot(to).clamp(-1.0, 1.0);
    let angle = dot.acos();
    if angle <= max_radians {
        return to;
    }
    let cross = from.x * to.y - from.y * to.x;
    let sign = if cross >= 0.0 { 1.0 } else { -1.0 };
    rotate(from, sign * max_radians)
}

fn rotate(vector: Vec2, angle_radians: f32) -> Vec2 {
    let (sin, cos) = angle_radians.sin_cos();
    Vec2::new(
        vector.x * cos - vector.y * sin,
        vector.x * sin + vector.y * cos,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_step_is_not_smoothed() {
        let mut state = MovementSmoothingState::default();
        let settings = MovementFeelSettings::default();
        let first = state.smooth_direction(UnitId::new(1), Vec2::new(1.0, 0.0), &settings);
        assert!((first.x - 1.0).abs() < 1e-4);
    }
}
