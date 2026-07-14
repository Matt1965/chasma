use bevy::prelude::*;

/// Access policy seam for future inventory interaction rules (ADR-087 I1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum InventoryAccessType {
    #[default]
    OwnerOnly,
    PartyShared,
    BuildingStorage,
    CorpseLoot,
}

impl InventoryAccessType {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "owner" | "owneronly" | "owner_only" => Ok(Self::OwnerOnly),
            "party" | "partyshared" | "party_shared" => Ok(Self::PartyShared),
            "building" | "buildingstorage" | "building_storage" => Ok(Self::BuildingStorage),
            "corpse" | "corpseloot" | "corpse_loot" => Ok(Self::CorpseLoot),
            other => Err(format!("unknown Access Type `{other}`")),
        }
    }
}
