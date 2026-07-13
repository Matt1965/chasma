/// In-memory unit fixtures for unit tests only. Runtime catalogs come from Excel import.
#[cfg(test)]
mod fixtures {
    use crate::world::unit::catalog::definition::UnitDefinition;
    use crate::world::unit::catalog::definition_id::UnitDefinitionId;
    use crate::world::unit::catalog::render_key::UnitRenderKey;
    use crate::world::unit::catalog::work::UnitWorkCapabilities;
    use crate::world::weapon::WeaponDefinitionId;

    pub fn starter_definitions() -> Vec<UnitDefinition> {
        vec![
            UnitDefinition::new(
                UnitDefinitionId::new("wolf"),
                "Wolf",
                "Wild",
                2,
                5,
                5,
                4,
                6,
                3,
                7,
                2,
                3,
                26.5,
                "Elite",
                4.5,
                0.6,
                40.0,
                WeaponDefinitionId::new("weapon_wolf_bite"),
                true,
                UnitRenderKey::reserved("wolf"),
            ),
            UnitDefinition::new(
                UnitDefinitionId::new("bandit"),
                "Bandit Scout",
                "Bandits",
                3,
                8,
                8,
                4,
                7,
                3,
                6,
                3,
                4,
                31.6,
                "Elite",
                3.8,
                0.45,
                35.0,
                WeaponDefinitionId::new("weapon_fists"),
                true,
                UnitRenderKey::reserved("bandit"),
            )
            .with_work_capabilities(UnitWorkCapabilities::builder(1.0)),
            UnitDefinition::new(
                UnitDefinitionId::new("deer"),
                "Deer",
                "Wild",
                1,
                4,
                4,
                2,
                5,
                2,
                8,
                1,
                2,
                12.0,
                "Common",
                5.5,
                0.5,
                30.0,
                WeaponDefinitionId::new("weapon_claws"),
                true,
                UnitRenderKey::reserved("deer"),
            ),
        ]
    }
}

#[cfg(test)]
pub use fixtures::starter_definitions;

#[cfg(not(test))]
pub fn starter_definitions() -> Vec<crate::world::unit::catalog::definition::UnitDefinition> {
    Vec::new()
}
