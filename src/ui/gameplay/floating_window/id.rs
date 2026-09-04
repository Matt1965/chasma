//! Stable identities for gameplay floating windows (BP5).

use bevy::prelude::*;

/// Secondary gameplay windows that may float independently of the fixed bottom HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum FloatingGameplayWindowId {
    BuildingMenu,
    UnitInventory,
    UnitSkills,
}

impl FloatingGameplayWindowId {
    pub const ALL: [Self; 3] = [Self::BuildingMenu, Self::UnitInventory, Self::UnitSkills];

    pub fn label(self) -> &'static str {
        match self {
            Self::BuildingMenu => "Building Menu",
            Self::UnitInventory => "Unit Inventory",
            Self::UnitSkills => "Unit Skills",
        }
    }
}
