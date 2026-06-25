use bevy::prelude::*;

/// Stable string identifier for a weapon type definition (ADR-054 C1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct WeaponDefinitionId(pub String);

impl WeaponDefinitionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for WeaponDefinitionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_definition_id_parses_and_compares() {
        let a = WeaponDefinitionId::new("weapon_fists");
        let b = WeaponDefinitionId::from("weapon_fists");
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "weapon_fists");
    }
}
