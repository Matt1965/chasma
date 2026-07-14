//! Unit inventory profile reference validation (ADR-087 I1).

use crate::world::{InventoryProfileCatalog, UnitDefinition, UnitDefinitionId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitInventoryProfileValidationError {
    MissingInventoryProfileReference {
        unit_id: UnitDefinitionId,
        profile_id: crate::world::InventoryProfileId,
    },
}

impl std::fmt::Display for UnitInventoryProfileValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingInventoryProfileReference {
                unit_id,
                profile_id,
            } => write!(
                f,
                "unit `{}` references missing inventory profile `{}`",
                unit_id.as_str(),
                profile_id.as_str()
            ),
        }
    }
}

pub fn validate_unit_inventory_profile_reference(
    unit: &UnitDefinition,
    profiles: &InventoryProfileCatalog,
) -> Result<(), UnitInventoryProfileValidationError> {
    let Some(profile_id) = &unit.inventory_profile_id else {
        return Ok(());
    };
    profiles
        .validate_profile_reference("unit", unit.id.as_str(), profile_id)
        .map_err(
            |_| UnitInventoryProfileValidationError::MissingInventoryProfileReference {
                unit_id: unit.id.clone(),
                profile_id: profile_id.clone(),
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{UnitDefinition, UnitDefinitionId, UnitRenderKey, WeaponDefinitionId};

    #[test]
    fn blank_profile_reference_means_no_inventory() {
        let unit = UnitDefinition::new(
            UnitDefinitionId::new("wolf"),
            "Wolf",
            "Wild",
            1,
            5,
            5,
            1,
            1,
            1,
            1,
            1,
            1,
            1.0,
            "normal",
            4.0,
            0.5,
            40.0,
            WeaponDefinitionId::new("weapon_fists"),
            true,
            UnitRenderKey::unset(),
        );
        assert!(unit.inventory_profile_id.is_none());
        validate_unit_inventory_profile_reference(&unit, &InventoryProfileCatalog::default())
            .expect("no profile is valid");
    }
}
