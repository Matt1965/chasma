use crate::world::armor::{ArmorProfileDefinition, ArmorProfileId};

pub const REQUIRED_COLUMNS: &[&str] = &[
    "Profile ID",
    "Name",
    "Description",
    "Armor Rating",
    "Enabled",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ArmorProfileImportRow {
    pub row_number: usize,
    pub profile_id: String,
    pub name: String,
    pub description: String,
    pub armor_rating: u32,
    pub enabled: bool,
    pub enabled_was_blank: bool,
}

impl ArmorProfileImportRow {
    pub fn to_definition(&self) -> ArmorProfileDefinition {
        ArmorProfileDefinition::new(
            ArmorProfileId::new(self.profile_id.trim()),
            self.name.trim(),
            self.description.trim(),
            self.armor_rating,
            self.enabled,
        )
    }
}
