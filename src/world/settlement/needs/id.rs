//! Stable NeedId (SA2).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable authored need identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize)]
pub struct NeedId(pub String);

impl NeedId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NeedId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for NeedId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
