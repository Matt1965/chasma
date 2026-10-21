//! Stable identities for gameplay floating windows (BP5).

use bevy::prelude::*;

/// Secondary gameplay windows that may float independently of the fixed bottom HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum FloatingGameplayWindowId {
    BuildingMenu,
    UnitInventory,
    UnitSkills,
    SettlementWorkforce,
    Dialogue,
    Trade,
    /// Second unit inventory during player-to-player transfer (paired with `UnitInventory`).
    PeerUnitInventory,
}

impl FloatingGameplayWindowId {
    pub const ALL: [Self; 7] = [
        Self::BuildingMenu,
        Self::UnitInventory,
        Self::PeerUnitInventory,
        Self::UnitSkills,
        Self::SettlementWorkforce,
        Self::Dialogue,
        Self::Trade,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::BuildingMenu => "Building Menu",
            Self::UnitInventory => "Unit Inventory",
            Self::UnitSkills => "Unit Skills",
            Self::SettlementWorkforce => "Settlement Workforce",
            Self::Dialogue => "Dialogue",
            Self::Trade => "Trade",
            Self::PeerUnitInventory => "Unit Inventory",
        }
    }
}
