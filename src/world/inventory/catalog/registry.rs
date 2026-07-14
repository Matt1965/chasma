use std::collections::HashMap;

use bevy::prelude::*;

use super::super::profile::InventoryProfileDefinition;
use super::super::profile_id::InventoryProfileId;
use super::super::validation::{InventoryProfileValidationError, validate_inventory_profile};
use super::starter::starter_definitions;

/// Read-only registry of inventory container profiles (ADR-087 I1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct InventoryProfileCatalog {
    definitions: Vec<InventoryProfileDefinition>,
    by_id: HashMap<InventoryProfileId, usize>,
}

impl Default for InventoryProfileCatalog {
    fn default() -> Self {
        #[cfg(any(test, feature = "dev"))]
        {
            Self::from_definitions(starter_definitions())
                .expect("inventory profile catalog is valid")
        }
        #[cfg(not(any(test, feature = "dev")))]
        {
            Self::from_definitions(Vec::new()).expect("empty inventory profile catalog is valid")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryProfileCatalogError {
    DuplicateId(InventoryProfileId),
    Validation(InventoryProfileValidationError),
    MissingInventoryProfileReference {
        owner_kind: &'static str,
        owner_id: String,
        profile_id: InventoryProfileId,
    },
    DisabledInventoryProfileReference {
        owner_kind: &'static str,
        owner_id: String,
        profile_id: InventoryProfileId,
    },
}

impl std::fmt::Display for InventoryProfileCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => {
                write!(f, "duplicate inventory profile id `{id}`", id = id.as_str())
            }
            Self::Validation(err) => write!(f, "{err}"),
            Self::MissingInventoryProfileReference {
                owner_kind,
                owner_id,
                profile_id,
            } => write!(
                f,
                "{owner_kind} `{owner_id}` references missing inventory profile `{}`",
                profile_id.as_str()
            ),
            Self::DisabledInventoryProfileReference {
                owner_kind,
                owner_id,
                profile_id,
            } => write!(
                f,
                "{owner_kind} `{owner_id}` references disabled inventory profile `{}`",
                profile_id.as_str()
            ),
        }
    }
}

impl InventoryProfileCatalog {
    pub fn from_definitions(
        definitions: Vec<InventoryProfileDefinition>,
    ) -> Result<Self, InventoryProfileCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());

        for definition in &definitions {
            validate_inventory_profile(definition, None)
                .map_err(InventoryProfileCatalogError::Validation)?;
        }

        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(InventoryProfileCatalogError::DuplicateId(
                    definition.id.clone(),
                ));
            }
        }

        Ok(Self { definitions, by_id })
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn definitions(&self) -> &[InventoryProfileDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &InventoryProfileId) -> Option<&InventoryProfileDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &InventoryProfileDefinition> {
        self.definitions
            .iter()
            .filter(|definition| definition.enabled)
    }

    pub fn validate_profile_reference(
        &self,
        owner_kind: &'static str,
        owner_id: &str,
        profile_id: &InventoryProfileId,
    ) -> Result<(), InventoryProfileCatalogError> {
        let Some(profile) = self.get(profile_id) else {
            return Err(
                InventoryProfileCatalogError::MissingInventoryProfileReference {
                    owner_kind,
                    owner_id: owner_id.to_string(),
                    profile_id: profile_id.clone(),
                },
            );
        };
        if !profile.enabled {
            return Err(
                InventoryProfileCatalogError::DisabledInventoryProfileReference {
                    owner_kind,
                    owner_id: owner_id.to_string(),
                    profile_id: profile_id.clone(),
                },
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_profiles_load() {
        let catalog = InventoryProfileCatalog::default();
        assert!(
            catalog
                .get(&InventoryProfileId::new("unit_backpack_standard"))
                .is_some()
        );
    }

    #[test]
    fn duplicate_profile_id_rejected() {
        let defs = starter_definitions();
        let mut dup = defs.clone();
        dup.push(defs[0].clone());
        assert!(matches!(
            InventoryProfileCatalog::from_definitions(dup),
            Err(InventoryProfileCatalogError::DuplicateId(_))
        ));
    }
}
