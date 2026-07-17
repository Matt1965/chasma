use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable identifier for a reusable terrain-field response curve (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, PartialOrd, Ord)]
pub struct FieldResponseProfileId(pub String);

impl FieldResponseProfileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FieldResponseProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn validate_field_response_profile_id(id: &str) -> Result<(), String> {
    let trimmed = id.trim();
    if trimmed.is_empty() || trimmed != trimmed.to_lowercase() {
        return Err(format!("invalid response profile id `{id}`"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!("invalid response profile id `{id}`"));
    }
    Ok(())
}
