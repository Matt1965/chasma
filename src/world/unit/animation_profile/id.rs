use std::sync::Arc;

use bevy::prelude::*;

/// Stable catalog key for a locomotion animation profile (A1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct AnimationProfileId(pub Arc<str>);

impl AnimationProfileId {
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AnimationProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
