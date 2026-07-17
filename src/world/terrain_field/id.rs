use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable string identifier for a terrain field (ADR-101 TF1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, PartialOrd, Ord)]
pub struct TerrainFieldId(pub String);

impl TerrainFieldId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TerrainFieldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Optional reference to an import/generation profile (TF2 seam).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, PartialOrd, Ord)]
pub struct TerrainFieldSourceProfileId(pub String);

impl TerrainFieldSourceProfileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TerrainFieldSourceProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
