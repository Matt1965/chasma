use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::id::RoadId;

/// Classification for future hybrid junction handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum RoadCrossingKind {
    /// Roads visually cross but are not connected for travel.
    NonConnected,
    /// Roads are connected for travel at this crossing.
    Connected,
}

/// Optional persisted override for incidental segment crossings.
///
/// Incidental crossings may also be derived later; this schema keeps explicit
/// non-connected crossings representable without a second authority.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct RoadCrossingOverride {
    pub road_a: RoadId,
    pub road_b: RoadId,
    pub kind: RoadCrossingKind,
    /// Normalized arc-length along `road_a`.
    pub road_a_t: f32,
    /// Normalized arc-length along `road_b`.
    pub road_b_t: f32,
}
