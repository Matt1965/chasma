//! Stable work-skill identifiers.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Semantic identifier for one authored work skill definition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize)]
pub struct WorkSkillId(pub String);

impl WorkSkillId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkSkillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for WorkSkillId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
