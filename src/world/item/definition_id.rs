use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable string identifier for an item type definition (ADR-087 I1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect, Serialize, Deserialize)]
pub struct ItemDefinitionId(pub String);

impl ItemDefinitionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ItemDefinitionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_definition_id_parses_and_compares() {
        let a = ItemDefinitionId::new("gold");
        let b = ItemDefinitionId::from("gold");
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "gold");
    }
}
