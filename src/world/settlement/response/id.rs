//! Stable ResponseId (SA3).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable authored response identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize)]
pub struct ResponseId(pub String);

impl ResponseId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResponseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ResponseId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
