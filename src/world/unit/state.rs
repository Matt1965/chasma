use bevy::prelude::*;

use crate::world::NavigationPath;

/// Top-level simulation phase for a unit instance (ADR-027 U2, ADR-030 U5, ADR-032 U7).
///
/// Broad enough for future combat, harvesting, and death. Locomotion orders are
/// represented here until a fuller [`super::UnitSimulationState`] envelope arrives.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub enum UnitState {
    #[default]
    Idle,
    Moving {
        target: crate::world::WorldPosition,
        path: NavigationPath,
        waypoint_index: usize,
    },
}
