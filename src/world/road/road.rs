use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::attachment::{RoadEndpointAttachment, RoadTeeAttachment};
use super::control_point::RoadControlPoint;
use super::error::RoadError;
use super::id::RoadId;
use super::style::{RoadStyleId, RoadStyleOverrides};

/// One authored road spline object.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct Road {
    pub id: RoadId,
    /// Author-facing label; does not replace [`RoadId`].
    #[serde(default)]
    pub display_name: String,
    pub style: RoadStyleId,
    pub style_overrides: RoadStyleOverrides,
    pub control_points: Vec<RoadControlPoint>,
    pub start_attachment: Option<RoadEndpointAttachment>,
    pub end_attachment: Option<RoadEndpointAttachment>,
    pub tee_attachments: Vec<RoadTeeAttachment>,
}

impl Road {
    pub fn validate(&self) -> Result<(), RoadError> {
        if self.id.as_str().is_empty() {
            return Err(RoadError::InvalidRoadId(
                "road id must not be empty".to_string(),
            ));
        }
        if self.control_points.len() < 2 {
            return Err(RoadError::InvalidControlPointCount {
                road_id: self.id.clone(),
                count: self.control_points.len(),
            });
        }
        for point in &self.control_points {
            point.validate()?;
        }
        self.style_overrides.validate()?;
        for tee in &self.tee_attachments {
            if !tee.host_t.is_finite() || !(0.0..=1.0).contains(&tee.host_t) {
                return Err(RoadError::InvalidJunctionReference(format!(
                    "road {} tee host_t must be finite and within [0, 1]",
                    self.id
                )));
            }
        }
        Ok(())
    }
}
