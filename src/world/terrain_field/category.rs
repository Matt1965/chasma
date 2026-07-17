use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Organizes terrain fields in UI and authoring only (ADR-101).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum TerrainFieldCategory {
    Hydrological,
    Geological,
    Agricultural,
    Atmospheric,
    Thermal,
    Other,
}

impl TerrainFieldCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hydrological => "Hydrological",
            Self::Geological => "Geological",
            Self::Agricultural => "Agricultural",
            Self::Atmospheric => "Atmospheric",
            Self::Thermal => "Thermal",
            Self::Other => "Other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "Hydrological" | "hydrological" => Some(Self::Hydrological),
            "Geological" | "geological" => Some(Self::Geological),
            "Agricultural" | "agricultural" => Some(Self::Agricultural),
            "Atmospheric" | "atmospheric" => Some(Self::Atmospheric),
            "Thermal" | "thermal" => Some(Self::Thermal),
            "Other" | "other" => Some(Self::Other),
            _ => None,
        }
    }
}
