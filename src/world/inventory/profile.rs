use bevy::prelude::*;

use super::access::InventoryAccessType;
use super::profile_id::InventoryProfileId;

/// Authoritative description of a fixed-grid inventory container profile (ADR-087 I1).
///
/// Weight fields are soft encumbrance metadata — they do not hard-reject placement.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct InventoryProfileDefinition {
    pub id: InventoryProfileId,
    pub display_name: String,
    pub grid_width: u8,
    pub grid_height: u8,
    pub reference_weight_grams: Option<u32>,
    pub comfortable_weight_grams: Option<u32>,
    pub encumbrance_threshold_grams: Option<u32>,
    pub global_stack_cap: Option<u32>,
    pub access_type: InventoryAccessType,
    pub enabled: bool,
}

impl InventoryProfileDefinition {
    pub fn new(
        id: InventoryProfileId,
        display_name: impl Into<String>,
        grid_width: u8,
        grid_height: u8,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            grid_width,
            grid_height,
            reference_weight_grams: None,
            comfortable_weight_grams: None,
            encumbrance_threshold_grams: None,
            global_stack_cap: None,
            access_type: InventoryAccessType::default(),
            enabled,
        }
    }

    pub fn with_reference_weight_grams(mut self, grams: u32) -> Self {
        self.reference_weight_grams = Some(grams);
        self
    }

    pub fn with_global_stack_cap(mut self, cap: u32) -> Self {
        self.global_stack_cap = Some(cap);
        self
    }

    pub fn with_access_type(mut self, access_type: InventoryAccessType) -> Self {
        self.access_type = access_type;
        self
    }
}
