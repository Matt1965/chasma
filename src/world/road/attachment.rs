use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::id::{JunctionId, RoadId};

/// Persisted endpoint attachment to a junction.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct RoadEndpointAttachment {
    pub junction_id: JunctionId,
    /// `true` for the road's first control point, `false` for the last.
    pub is_start: bool,
}

/// Persisted tee attachment where a road meets another road at an interior point.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct RoadTeeAttachment {
    pub junction_id: JunctionId,
    /// Road that owns the tee intersection along its spline.
    pub host_road_id: RoadId,
    /// Normalized arc-length parameter in `[0, 1]` along `host_road_id`.
    pub host_t: f32,
}
