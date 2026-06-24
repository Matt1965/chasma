//! Unit formation planning for group movement (ADR-035 U10).
//!
//! Spatial decomposition of a single click target into per-unit move destinations.
//! Does not perform pathfinding or obstacle reasoning.

mod distribution;
mod layout;
mod offsets;
mod planner;

pub use distribution::{circle_formation_radius, formation_offsets};
pub use layout::FormationKind;
pub use offsets::unit_spacing_meters;
pub use planner::{FormationAssignment, FormationMovePlan, FormationPlanner};
