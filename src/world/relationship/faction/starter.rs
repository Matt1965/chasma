/// In-memory faction fixtures for unit tests only. Runtime catalogs come from Excel import.
#[cfg(test)]
mod fixtures {
    use super::super::definition::FactionDefinition;
    use super::super::id::FactionId;

    pub fn starter_definitions() -> Vec<FactionDefinition> {
        vec![
            FactionDefinition::new(
                FactionId::new("player"),
                "Player",
                "The player's faction",
                true,
            )
            .with_legacy_faction_id("F-0001"),
            FactionDefinition::new(
                FactionId::new("wild"),
                "Wild",
                "Wild creatures and beasts",
                true,
            )
            .with_legacy_faction_id("F-0002"),
            FactionDefinition::new(
                FactionId::new("bandits"),
                "Bandits",
                "Opportunistic raiders",
                true,
            )
            .with_legacy_faction_id("F-0003"),
        ]
    }
}

#[cfg(test)]
pub use fixtures::starter_definitions;

#[cfg(not(test))]
pub fn starter_definitions()
-> Vec<crate::world::relationship::faction::definition::FactionDefinition> {
    Vec::new()
}
