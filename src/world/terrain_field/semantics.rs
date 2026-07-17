use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Documents how field values should be interpreted (ADR-101).
///
/// Does not branch storage or query behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum FieldValueSemantics {
    EnvironmentalPotential,
    GeologicalPotential,
    Suitability,
    /// Seam for future derived/dynamic fields (e.g. wind).
    DerivedDynamic,
}

impl FieldValueSemantics {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnvironmentalPotential => "EnvironmentalPotential",
            Self::GeologicalPotential => "GeologicalPotential",
            Self::Suitability => "Suitability",
            Self::DerivedDynamic => "DerivedDynamic",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "EnvironmentalPotential" | "environmental_potential" => {
                Some(Self::EnvironmentalPotential)
            }
            "GeologicalPotential" | "geological_potential" => Some(Self::GeologicalPotential),
            "Suitability" | "suitability" => Some(Self::Suitability),
            "DerivedDynamic" | "derived_dynamic" => Some(Self::DerivedDynamic),
            _ => None,
        }
    }
}
