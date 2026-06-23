//! Local unit steering for cohesion and separation (ADR-036 U11).
//!
//! Adjusts movement direction after pathfinding without modifying paths or waypoints.

mod alignment;
mod avoidance;
mod cohesion;
mod separation;

pub use avoidance::{apply_steering, gather_steering_neighbors, SteeringNeighbor};
pub use cohesion::cohesion_force;
pub use separation::separation_force;

/// Conservative tuning for local steering forces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteeringSettings {
    pub separation_strength: f32,
    pub separation_radius_padding: f32,
    pub max_separation_force: f32,
    pub min_separation_distance: f32,
    pub cohesion_strength: f32,
    pub cohesion_arrival_threshold_sq: f32,
    pub alignment_strength: f32,
    pub max_steering_influence: f32,
    pub max_steering_angle_radians: f32,
    pub neighbor_query_radius: f32,
}

impl Default for SteeringSettings {
    fn default() -> Self {
        Self {
            separation_strength: 1.2,
            separation_radius_padding: 0.25,
            max_separation_force: 0.85,
            min_separation_distance: 0.05,
            cohesion_strength: 0.08,
            cohesion_arrival_threshold_sq: 0.15_f32.powi(2),
            alignment_strength: 0.04,
            max_steering_influence: 0.35,
            max_steering_angle_radians: 35.0_f32.to_radians(),
            neighbor_query_radius: 6.0,
        }
    }
}

impl SteeringSettings {
    pub const DEFAULT: Self = Self {
        separation_strength: 1.2,
        separation_radius_padding: 0.25,
        max_separation_force: 0.85,
        min_separation_distance: 0.05,
        cohesion_strength: 0.08,
        cohesion_arrival_threshold_sq: 0.0225,
        alignment_strength: 0.04,
        max_steering_influence: 0.35,
        max_steering_angle_radians: 0.61086524,
        neighbor_query_radius: 6.0,
    };
}
