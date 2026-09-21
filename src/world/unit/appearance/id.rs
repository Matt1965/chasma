use bevy::prelude::*;

/// Authoritative appearance profile id (workbook `Appearance Profiles` sheet).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct AppearanceProfileId(pub String);

impl AppearanceProfileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Body variant within an appearance profile (workbook `Appearance Body Variants`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct BodyVariantId(pub String);

impl BodyVariantId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic appearance parameter id (workbook `Appearance Parameters`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Reflect)]
pub struct AppearanceParamId(pub String);

impl AppearanceParamId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
