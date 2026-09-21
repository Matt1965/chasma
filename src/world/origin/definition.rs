use bevy::prelude::Reflect;
use serde::{Deserialize, Serialize};

use super::id::OriginId;
use super::snapshot::OriginSquadMemberSnapshot;

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct OriginDefinition {
    pub id: OriginId,
    pub display_name: String,
    pub description: String,
    pub members: Vec<OriginSquadMemberSnapshot>,
    pub start_x: f32,
    pub start_z: f32,
    pub yaw_deg: f32,
}

impl OriginDefinition {
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn spawn_anchor(&self) -> super::spawn::OriginSpawnAnchor {
        super::spawn::OriginSpawnAnchor {
            start_x: self.start_x,
            start_z: self.start_z,
            yaw_deg: self.yaw_deg,
        }
    }
}
