//! Broad purpose classification for building inventories (EP4).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Broad purpose of a building-owned inventory (EP4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum BuildingInventoryRole {
    General,
    Input,
    Output,
    Fuel,
    Waste,
    Catalyst,
}

impl BuildingInventoryRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Input => "Input",
            Self::Output => "Output",
            Self::Fuel => "Fuel",
            Self::Waste => "Waste",
            Self::Catalyst => "Catalyst",
        }
    }

    /// Whether an operation input may target this role (EP4 validation).
    pub fn accepts_operation_input(self) -> bool {
        matches!(
            self,
            Self::Input | Self::Fuel | Self::Catalyst | Self::General
        )
    }

    /// Whether an operation output may target this role (EP4 validation).
    pub fn accepts_operation_output(self) -> bool {
        matches!(self, Self::Output | Self::Waste | Self::General)
    }

    /// Whether this binding may supply items to remote buildings via logistics (EP8).
    pub fn advertises_logistics_supply(self) -> bool {
        matches!(self, Self::Output | Self::Waste | Self::General)
    }

    /// Whether this binding may receive inbound logistics deliveries (EP8).
    pub fn accepts_logistics_delivery(self) -> bool {
        matches!(
            self,
            Self::Input | Self::Fuel | Self::Catalyst | Self::General
        )
    }
}
