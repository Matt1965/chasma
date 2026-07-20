//! Authoritative ConstructionPlan model (SA9 / ADR-124).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::building::catalog::BuildingDefinitionId;
use crate::world::item::ItemDefinitionId;
use crate::world::settlement::arbiter::IntentId;
use crate::world::settlement::response::ResponseId;
use crate::world::settlement::SettlementId;
use crate::world::{BuildingId, ChunkCoord, LocalPosition, WorldPosition};

/// Stable construction plan identifier (persisted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize)]
pub struct ConstructionPlanId(pub u64);

impl ConstructionPlanId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Construction plan lifecycle. Names align with building/task language already in-tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstructionPlanStatus {
    Proposed,
    SiteSearch,
    AwaitingApproval,
    AwaitingMaterials,
    Ready,
    InProgress,
    Blocked,
    Completed,
    Cancelled,
    Superseded,
}

impl ConstructionPlanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::SiteSearch => "site_search",
            Self::AwaitingApproval => "awaiting_approval",
            Self::AwaitingMaterials => "awaiting_materials",
            Self::Ready => "ready",
            Self::InProgress => "in_progress",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Superseded
        )
    }

    /// Committed plans survive brief intent/pressure fluctuation.
    pub fn is_committed(self) -> bool {
        matches!(
            self,
            Self::AwaitingApproval
                | Self::AwaitingMaterials
                | Self::Ready
                | Self::InProgress
                | Self::Blocked
        )
    }

    pub fn is_active(self) -> bool {
        !self.is_terminal()
    }
}

/// Durable source reference once SettlementIntent is gone.
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct ConstructionPlanSource {
    pub response_id: ResponseId,
    pub intent_id: Option<String>,
    pub source_need: Option<String>,
}

impl ConstructionPlanSource {
    pub fn from_intent(
        response_id: ResponseId,
        intent_id: &IntentId,
        source_need: impl Into<String>,
    ) -> Self {
        Self {
            response_id,
            intent_id: Some(intent_id.as_str().to_string()),
            source_need: Some(source_need.into()),
        }
    }

    pub fn manual(response_id: ResponseId) -> Self {
        Self {
            response_id,
            intent_id: None,
            source_need: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ConstructionMaterialRequirement {
    pub item_id: ItemDefinitionId,
    pub required: u32,
    pub delivered: u32,
}

impl ConstructionMaterialRequirement {
    pub fn new(item_id: ItemDefinitionId, required: u32) -> Self {
        Self {
            item_id,
            required,
            delivered: 0,
        }
    }

    pub fn missing(&self) -> u32 {
        self.required.saturating_sub(self.delivered)
    }
}

/// Placement candidate with serde-friendly coordinates (WorldPosition is not serialized).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ConstructionPlacementCandidate {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub local_x: f32,
    pub local_y: f32,
    pub local_z: f32,
    pub yaw_quadrants: u8,
    pub hard_valid: bool,
    pub soft_score: i32,
    pub hard_reject_reason: Option<String>,
}

impl ConstructionPlacementCandidate {
    pub fn from_world_position(
        position: WorldPosition,
        yaw_quadrants: u8,
        soft_score: i32,
    ) -> Self {
        Self {
            chunk_x: position.chunk.x,
            chunk_z: position.chunk.z,
            local_x: position.local.0.x,
            local_y: position.local.0.y,
            local_z: position.local.0.z,
            yaw_quadrants,
            hard_valid: true,
            soft_score,
            hard_reject_reason: None,
        }
    }

    pub fn position(&self) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(self.chunk_x, self.chunk_z),
            LocalPosition::new(Vec3::new(self.local_x, self.local_y, self.local_z)),
        )
    }
}

/// Authoritative committed construction work for a settlement.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ConstructionPlan {
    pub id: ConstructionPlanId,
    pub settlement_id: SettlementId,
    pub source: ConstructionPlanSource,
    pub building_definition_id: BuildingDefinitionId,
    /// Capability / purpose string (operation id, category id, or mapping key).
    pub required_capability: String,
    pub fulfillment_key: String,
    pub placement: Option<ConstructionPlacementCandidate>,
    pub reserved_building_id: Option<BuildingId>,
    pub priority: f32,
    pub required_materials: Vec<ConstructionMaterialRequirement>,
    pub status: ConstructionPlanStatus,
    pub blocking_reason: Option<String>,
    pub created_tick: u64,
    pub updated_tick: u64,
    pub next_retry_tick: Option<u64>,
    pub retry_count: u32,
    pub player_approved: bool,
    pub diagnostics: Vec<String>,
}

impl ConstructionPlan {
    pub fn materials_satisfied(&self) -> bool {
        self.required_materials.iter().all(|m| m.missing() == 0)
    }

    pub fn refresh_priority(&mut self, priority: f32, tick: u64) {
        self.priority = priority;
        self.updated_tick = tick;
    }
}

/// Persisted construction plan store payload.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConstructionPlanSaveState {
    pub plans: Vec<ConstructionPlan>,
    pub next_plan_id: u64,
}
