/// Starter weapon fixtures for unit tests only.
#[cfg(any(test, feature = "dev"))]
mod fixtures {
    use crate::world::{DamageType, HitMode, TargetFilter, WeaponDefinition, WeaponDefinitionId};

    pub fn starter_definitions() -> Vec<WeaponDefinition> {
        vec![
            WeaponDefinition::new(
                WeaponDefinitionId::new("weapon_fists"),
                "Fists",
                "Unarmed strikes.",
                4.0,
                DamageType::Blunt,
                1.2,
                1.5,
                0.15,
                0.1,
                HitMode::Melee,
                None,
                0.0,
                "attack_fists",
                vec![TargetFilter::Enemies],
                None,
                true,
            ),
            WeaponDefinition::new(
                WeaponDefinitionId::new("weapon_wolf_bite"),
                "Wolf Bite",
                "Natural wolf bite attack.",
                8.0,
                DamageType::Slashing,
                1.5,
                1.2,
                0.2,
                0.15,
                HitMode::Melee,
                None,
                0.0,
                "attack_bite",
                vec![TargetFilter::Enemies, TargetFilter::Wildlife],
                None,
                true,
            ),
            WeaponDefinition::new(
                WeaponDefinitionId::new("weapon_claws"),
                "Claws",
                "Natural claw swipe.",
                6.0,
                DamageType::Slashing,
                1.4,
                1.25,
                0.18,
                0.12,
                HitMode::Melee,
                None,
                0.0,
                "attack_claws",
                vec![TargetFilter::Enemies, TargetFilter::Wildlife],
                None,
                true,
            ),
        ]
    }
}

#[cfg(any(test, feature = "dev"))]
pub use fixtures::starter_definitions;

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_definitions() -> Vec<crate::world::WeaponDefinition> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::starter_definitions;
    use crate::world::UnitCatalog;

    #[test]
    fn starter_units_reference_starter_weapons() {
        let weapons =
            super::super::registry::WeaponCatalog::from_definitions(starter_definitions()).unwrap();
        let units = UnitCatalog::default();
        weapons.validate_units(&units).expect("starter cross-refs");
    }
}
