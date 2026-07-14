use bevy::prelude::*;

/// Future UI icon lookup key for item definitions (ADR-087 I1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
pub struct ItemIconKey(pub Option<String>);

impl ItemIconKey {
    pub fn unset() -> Self {
        Self(None)
    }

    pub fn reserved(key: impl Into<String>) -> Self {
        Self(Some(key.into()))
    }
}
