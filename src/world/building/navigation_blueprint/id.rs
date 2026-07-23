use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable identifier for a building navigation blueprint (NV1.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, PartialOrd, Ord)]
pub struct BuildingNavigationBlueprintId(pub String);

impl BuildingNavigationBlueprintId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BuildingNavigationBlueprintId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for BuildingNavigationBlueprintId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for BuildingNavigationBlueprintId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn validate_navigation_blueprint_id(id: &str) -> Result<(), String> {
    let trimmed = id.trim();
    if trimmed.is_empty() || trimmed != trimmed.to_lowercase() {
        return Err(format!("invalid navigation blueprint id `{id}`"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!("invalid navigation blueprint id `{id}`"));
    }
    Ok(())
}
