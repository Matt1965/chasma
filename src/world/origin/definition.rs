use bevy::prelude::*;

use crate::world::UnitDefinitionId;

use super::id::OriginId;

/// One preview/spawn slot in an origin roster (CG7).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct OriginRosterMember {
    pub role_label: String,
    pub definition_id: UnitDefinitionId,
    /// Local preview-studio placement for multi-actor framing.
    pub preview_offset: Vec3,
}

/// Authored starting-origin data (CG7). Appearance defaults resolve from unit definitions.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct OriginDefinition {
    pub id: OriginId,
    pub display_name: String,
    pub description: String,
    pub roster: Vec<OriginRosterMember>,
}

impl OriginDefinition {
    pub fn roster_size(&self) -> usize {
        self.roster.len()
    }
}
