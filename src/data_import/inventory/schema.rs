//! Excel column schema for inventory profiles (ADR-087 I1).

use crate::world::{InventoryAccessType, InventoryProfileDefinition, InventoryProfileId};

pub const REQUIRED_COLUMNS: &[&str] = &[
    "Inventory Profile ID",
    "Name",
    "Grid Width",
    "Grid Height",
    "Enabled",
];

pub const OPTIONAL_COLUMNS: &[&str] =
    &["Reference Weight Grams", "Global Stack Cap", "Access Type"];

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryProfileImportRow {
    pub row_number: usize,
    pub profile_id: String,
    pub name: String,
    pub grid_width: u8,
    pub grid_height: u8,
    pub reference_weight_grams: Option<u32>,
    pub global_stack_cap: Option<u32>,
    pub access_type: InventoryAccessType,
    pub enabled: bool,
    pub enabled_was_blank: bool,
}

impl InventoryProfileImportRow {
    pub fn to_definition(&self) -> InventoryProfileDefinition {
        let mut definition = InventoryProfileDefinition::new(
            InventoryProfileId::new(self.profile_id.trim()),
            self.name.trim(),
            self.grid_width,
            self.grid_height,
            self.enabled,
        )
        .with_access_type(self.access_type);

        if let Some(grams) = self.reference_weight_grams {
            definition = definition.with_reference_weight_grams(grams);
        }
        if let Some(cap) = self.global_stack_cap {
            definition = definition.with_global_stack_cap(cap);
        }

        definition
    }
}
