use bevy::prelude::*;

/// Future rendering asset lookup key (ADR-016).
///
/// Placeholder only: does not load assets or reference the asset pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
pub struct DoodadRenderKey(pub Option<String>);

impl DoodadRenderKey {
    pub fn unset() -> Self {
        Self(None)
    }

    pub fn reserved(key: impl Into<String>) -> Self {
        Self(Some(key.into()))
    }

    pub fn as_str(&self) -> Option<&str> {
        self.0.as_deref()
    }
}
