use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable identifier for one authored road.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, PartialOrd, Ord)]
pub struct RoadId(pub String);

impl RoadId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RoadId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable identifier for one persisted road junction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, PartialOrd, Ord)]
pub struct JunctionId(pub String);

impl JunctionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for JunctionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
