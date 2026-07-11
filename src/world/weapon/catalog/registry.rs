use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::unit::UnitDefinition;

use super::definition::WeaponDefinition;
use super::definition_id::WeaponDefinitionId;
use super::starter::starter_definitions;

/// Read-only registry of weapon type definitions (ADR-054 C1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct WeaponCatalog {
    definitions: Vec<WeaponDefinition>,
    by_id: HashMap<WeaponDefinitionId, usize>,
}

impl Default for WeaponCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_definitions()).expect("weapon catalog is valid")
    }
}

/// Why [`WeaponCatalog::from_definitions`] or validation rejected input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeaponCatalogError {
    DuplicateId(WeaponDefinitionId),
    UnitWeaponNotFound {
        unit_id: crate::world::UnitDefinitionId,
        weapon_id: WeaponDefinitionId,
    },
    UnitWeaponDisabled {
        unit_id: crate::world::UnitDefinitionId,
        weapon_id: WeaponDefinitionId,
    },
}

impl std::fmt::Display for WeaponCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate weapon id `{id}`", id = id.as_str()),
            Self::UnitWeaponNotFound { unit_id, weapon_id } => write!(
                f,
                "unit `{}` references missing weapon `{}`",
                unit_id.as_str(),
                weapon_id.as_str()
            ),
            Self::UnitWeaponDisabled { unit_id, weapon_id } => write!(
                f,
                "unit `{}` references disabled weapon `{}`",
                unit_id.as_str(),
                weapon_id.as_str()
            ),
        }
    }
}

impl WeaponCatalog {
    pub fn from_definitions(
        definitions: Vec<WeaponDefinition>,
    ) -> Result<Self, WeaponCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());

        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(WeaponCatalogError::DuplicateId(definition.id.clone()));
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

    pub fn definitions(&self) -> &[WeaponDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &WeaponDefinitionId) -> Option<&WeaponDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &WeaponDefinition> {
        self.definitions
            .iter()
            .filter(|definition| definition.enabled)
    }

    /// Whether an enabled unit's default weapon exists and is enabled.
    pub fn validate_unit_default_weapon(
        &self,
        unit: &UnitDefinition,
    ) -> Result<(), WeaponCatalogError> {
        if !unit.enabled {
            return Ok(());
        }
        let weapon_id = &unit.default_weapon_id;
        let Some(weapon) = self.get(weapon_id) else {
            return Err(WeaponCatalogError::UnitWeaponNotFound {
                unit_id: unit.id.clone(),
                weapon_id: weapon_id.clone(),
            });
        };
        if !weapon.enabled {
            return Err(WeaponCatalogError::UnitWeaponDisabled {
                unit_id: unit.id.clone(),
                weapon_id: weapon_id.clone(),
            });
        }
        Ok(())
    }

    /// Validate every enabled unit references an enabled weapon.
    pub fn validate_units(
        &self,
        units: &crate::world::UnitCatalog,
    ) -> Result<(), WeaponCatalogError> {
        for unit in units.enabled_definitions() {
            self.validate_unit_default_weapon(unit)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{UnitDefinition, UnitDefinitionId, UnitRenderKey};

    #[test]
    fn catalog_contains_starter_weapons() {
        let catalog = WeaponCatalog::default();
        assert!(
            catalog
                .get(&WeaponDefinitionId::new("weapon_fists"))
                .is_some()
        );
        assert!(
            catalog
                .get(&WeaponDefinitionId::new("weapon_wolf_bite"))
                .is_some()
        );
        assert!(
            catalog
                .get(&WeaponDefinitionId::new("weapon_claws"))
                .is_some()
        );
    }

    #[test]
    fn lookup_by_weapon_id() {
        let catalog = WeaponCatalog::default();
        let fists = catalog
            .get(&WeaponDefinitionId::new("weapon_fists"))
            .unwrap();
        assert_eq!(fists.display_name, "Fists");
        assert!(catalog.get(&WeaponDefinitionId::new("missing")).is_none());
    }

    #[test]
    fn duplicate_weapon_id_rejected() {
        let mut defs = starter_definitions();
        defs.push(defs[0].clone());
        assert!(matches!(
            WeaponCatalog::from_definitions(defs),
            Err(WeaponCatalogError::DuplicateId(id)) if id.as_str() == "weapon_fists"
        ));
    }

    #[test]
    fn deterministic_iteration() {
        let catalog = WeaponCatalog::default();
        let ids_a: Vec<_> = catalog
            .definitions()
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        let ids_b: Vec<_> = catalog
            .definitions()
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(ids_a, ids_b);
    }

    #[test]
    fn disabled_weapon_excluded_from_enabled_iterator() {
        let mut defs = starter_definitions();
        defs[0].enabled = false;
        let catalog = WeaponCatalog::from_definitions(defs).unwrap();
        assert_eq!(catalog.enabled_definitions().count(), 2);
    }

    #[test]
    fn enabled_unit_missing_weapon_fails_validation() {
        let catalog = WeaponCatalog::default();
        let unit = UnitDefinition::new(
            UnitDefinitionId::new("wolf"),
            "Wolf",
            "Wild",
            1,
            5,
            5,
            4,
            6,
            3,
            7,
            2,
            3,
            10.0,
            "Common",
            4.0,
            0.5,
            40.0,
            WeaponDefinitionId::new("weapon_missing"),
            true,
            UnitRenderKey::unset(),
        );
        assert!(matches!(
            catalog.validate_unit_default_weapon(&unit),
            Err(WeaponCatalogError::UnitWeaponNotFound { .. })
        ));
    }

    #[test]
    fn enabled_unit_disabled_weapon_fails_validation() {
        let mut defs = starter_definitions();
        defs[0].enabled = false;
        let catalog = WeaponCatalog::from_definitions(defs).unwrap();
        let unit = UnitDefinition::new(
            UnitDefinitionId::new("wolf"),
            "Wolf",
            "Wild",
            1,
            5,
            5,
            4,
            6,
            3,
            7,
            2,
            3,
            10.0,
            "Common",
            4.0,
            0.5,
            40.0,
            WeaponDefinitionId::new("weapon_fists"),
            true,
            UnitRenderKey::unset(),
        );
        assert!(matches!(
            catalog.validate_unit_default_weapon(&unit),
            Err(WeaponCatalogError::UnitWeaponDisabled { .. })
        ));
    }
}
