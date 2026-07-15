use bevy::prelude::*;

/// Future rendering asset lookup key (ADR-078 B1, ADR-095 BA1).
///
/// Resolved at runtime to `assets/buildings/{key}.glb` by [`crate::buildings::BuildingSceneAssets`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
pub struct BuildingRenderKey(pub Option<String>);

impl BuildingRenderKey {
    pub fn unset() -> Self {
        Self(None)
    }

    pub fn reserved(key: impl Into<String>) -> Self {
        Self(Some(key.into()))
    }
}
