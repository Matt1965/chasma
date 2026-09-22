//! Minimal dialogue content representation.

use bevy::prelude::*;

/// Extensible authored/runtime dialogue body.
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Default)]
pub struct DialogueContent {
    pub lines: Vec<String>,
}

impl DialogueContent {
    pub fn single(line: impl Into<String>) -> Self {
        Self {
            lines: vec![line.into()],
        }
    }

    pub fn default_greeting() -> Self {
        Self::single("Hi.")
    }
}
