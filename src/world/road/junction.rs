use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::error::RoadError;
use super::id::{JunctionId, RoadId};

/// Role of a road member within a persisted junction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum JunctionMemberRole {
    EndpointStart,
    EndpointEnd,
    TeeHost,
    TeeBranch,
}

/// One road membership entry in a persisted junction.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct JunctionMember {
    pub road_id: RoadId,
    pub role: JunctionMemberRole,
    /// Normalized arc-length along `road_id` when `role` is `TeeHost`.
    pub host_t: Option<f32>,
}

impl JunctionMember {
    pub fn validate(&self) -> Result<(), RoadError> {
        match self.role {
            JunctionMemberRole::TeeHost => {
                let host_t = self.host_t.ok_or_else(|| {
                    RoadError::InvalidJunctionReference(format!(
                        "junction member for road {} requires host_t for TeeHost",
                        self.road_id
                    ))
                })?;
                if !host_t.is_finite() || !(0.0..=1.0).contains(&host_t) {
                    return Err(RoadError::InvalidJunctionReference(format!(
                        "junction member for road {} has invalid host_t",
                        self.road_id
                    )));
                }
            }
            JunctionMemberRole::EndpointStart | JunctionMemberRole::EndpointEnd => {
                if self.host_t.is_some() {
                    return Err(RoadError::InvalidJunctionReference(format!(
                        "junction member for road {} must not set host_t for endpoint roles",
                        self.road_id
                    )));
                }
            }
            JunctionMemberRole::TeeBranch => {
                if self.host_t.is_some() {
                    return Err(RoadError::InvalidJunctionReference(format!(
                        "junction member for road {} must not set host_t for TeeBranch",
                        self.road_id
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Persisted junction metadata for intentional endpoint/tee connections.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct Junction {
    pub id: JunctionId,
    pub members: Vec<JunctionMember>,
}

impl Junction {
    pub fn validate(&self) -> Result<(), RoadError> {
        if self.id.as_str().is_empty() {
            return Err(RoadError::InvalidJunctionId(
                "junction id must not be empty".to_string(),
            ));
        }
        if self.members.is_empty() {
            return Err(RoadError::InvalidJunctionReference(format!(
                "junction {} must have at least one member",
                self.id
            )));
        }
        for member in &self.members {
            member.validate()?;
        }
        Ok(())
    }
}
