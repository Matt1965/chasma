use bevy::prelude::*;

/// Stable string identifier for an item category (ADR-087 I1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct ItemCategoryId(pub String);

impl ItemCategoryId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ItemCategoryId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
