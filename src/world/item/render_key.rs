use bevy::prelude::*;

/// Future rendering asset lookup key for item meshes (ADR-087 I1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
pub struct ItemRenderKey(pub Option<String>);

impl ItemRenderKey {
    pub fn unset() -> Self {
        Self(None)
    }

    pub fn reserved(key: impl Into<String>) -> Self {
        Self(Some(key.into()))
    }
}
