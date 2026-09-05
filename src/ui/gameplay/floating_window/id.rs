//! Stable identities for gameplay floating windows (BP5).

use bevy::prelude::*;

/// Secondary gameplay windows that may float independently of the fixed bottom HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum FloatingGameplayWindowId {
    BuildingMenu,
    UnitInventory,
    UnitSkills,
    SettlementWorkforce,
}

impl FloatingGameplayWindowId {
    pub const ALL: [Self; 4] = [
        Self::BuildingMenu,
        Self::UnitInventory,
        Self::UnitSkills,
        Self::SettlementWorkforce,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::BuildingMenu => "Building Menu",
            Self::UnitInventory => "Unit Inventory",
            Self::UnitSkills => "Unit Skills",
            Self::SettlementWorkforce => "Settlement Workforce",
        }
    }
}
