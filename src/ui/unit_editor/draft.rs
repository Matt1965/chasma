//! Reusable unit appearance draft boundary (CG3/CG8).

use bevy::prelude::*;

use crate::world::{UnitAppearance, UnitDefinitionId};

/// Optional equipment snapshot for preview-only presentation (CG3 reads live unit; CG8+ may author).
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
pub struct EquipmentPreviewLoadout {
    /// Reserved for future equipment preview snapshots. Unused in CG3.
    pub placeholder: bool,
}

/// Editable unit appearance state — not authoritative [`WorldData`].
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct UnitAppearanceDraft {
    pub definition_id: UnitDefinitionId,
    pub display_name: Option<String>,
    pub appearance: UnitAppearance,
    pub preview_loadout: Option<EquipmentPreviewLoadout>,
}

impl UnitAppearanceDraft {
    pub fn from_live(
        definition_id: UnitDefinitionId,
        display_name: Option<String>,
        appearance: UnitAppearance,
    ) -> Self {
        Self {
            definition_id,
            display_name,
            appearance,
            preview_loadout: None,
        }
    }
}
