/// In-memory species fixtures for unit tests only. Runtime catalogs come from Excel import.
#[cfg(test)]
mod fixtures {
    use super::super::definition::SpeciesDefinition;
    use super::super::id::SpeciesId;

    pub fn starter_definitions() -> Vec<SpeciesDefinition> {
        vec![
            SpeciesDefinition::new(SpeciesId::new("wolf"), "Wolf", "Canine predator", true),
            SpeciesDefinition::new(SpeciesId::new("human"), "Human", "Baseline humanoid", true),
            SpeciesDefinition::new(SpeciesId::new("deer"), "Deer", "Grazing herbivore", true),
            SpeciesDefinition::new(
                SpeciesId::new("robot"),
                "Robot",
                "Player humanoid construct",
                true,
            ),
            SpeciesDefinition::new(SpeciesId::new("fox"), "Fox", "Small canid", true),
            SpeciesDefinition::new(
                SpeciesId::new("cavecrawler"),
                "Cavecrawler",
                "Subterranean predator",
                true,
            ),
        ]
    }
}

#[cfg(test)]
pub use fixtures::starter_definitions;

#[cfg(not(test))]
pub fn starter_definitions()
-> Vec<crate::world::relationship::species::definition::SpeciesDefinition> {
    Vec::new()
}
