use bevy::prelude::*;

use super::id::ItemInstanceId;
use crate::world::ItemDefinitionId;

/// Optional metadata on a unique item instance (ADR-088 I2).
///
/// Condition, durability, and repair state are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq, Default, Reflect)]
pub struct ItemInstanceMetadata {
    /// Reserved quality tier seam — no generation logic in I2.
    pub quality: Option<u32>,
}

/// Authoritative unique item instance (ADR-088 I2).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ItemInstance {
    pub id: ItemInstanceId,
    pub definition_id: ItemDefinitionId,
    pub metadata: ItemInstanceMetadata,
}

impl ItemInstance {
    pub fn new(id: ItemInstanceId, definition_id: ItemDefinitionId) -> Self {
        Self {
            id,
            definition_id,
            metadata: ItemInstanceMetadata::default(),
        }
    }

    pub fn with_metadata(mut self, metadata: ItemInstanceMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}
