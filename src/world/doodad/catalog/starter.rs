/// In-memory doodad fixtures for unit tests only. Runtime catalogs come from Excel import.
#[cfg(test)]
mod fixtures {
    use crate::world::doodad::catalog::definition::DoodadDefinition;
    use crate::world::doodad::catalog::definition_id::DoodadDefinitionId;
    use crate::world::doodad::catalog::render_key::DoodadRenderKey;
    use crate::world::biome::BiomeId;
    use crate::world::DoodadKind;

    fn all_assigned_biomes() -> Vec<BiomeId> {
        BiomeId::all_assigned().to_vec()
    }

    fn forest_biomes() -> Vec<BiomeId> {
        vec![BiomeId::Forest]
    }

    fn non_forest_biomes() -> Vec<BiomeId> {
        vec![BiomeId::Desert, BiomeId::Marsh, BiomeId::Plains]
    }

    fn with_random_rotation_y(definition: DoodadDefinition) -> DoodadDefinition {
        definition.with_random_rotation_y(true)
    }

    pub fn starter_definitions() -> Vec<DoodadDefinition> {
        vec![
            with_random_rotation_y(
                DoodadDefinition::new(
                    DoodadDefinitionId::new("tree_oak"),
                    DoodadKind::Tree,
                    "Oak Tree",
                    4.0,
                    0.85,
                    1.15,
                    None,
                    None,
                    Some(25.0),
                    true,
                    DoodadRenderKey::reserved("tree/oak"),
                )
                .with_allowed_biomes(forest_biomes())
                .with_spawn_weight(8.0)
                .with_block_radius_meters(1.0),
            ),
            with_random_rotation_y(
                DoodadDefinition::new(
                    DoodadDefinitionId::new("tree_dead"),
                    DoodadKind::Tree,
                    "Dead Tree",
                    3.5,
                    0.9,
                    1.1,
                    None,
                    None,
                    Some(30.0),
                    true,
                    DoodadRenderKey::reserved("tree/dead"),
                )
                .with_allowed_biomes(forest_biomes())
                .with_spawn_weight(2.0)
                .with_block_radius_meters(0.9),
            ),
            DoodadDefinition::new(
                DoodadDefinitionId::new("rock_small"),
                DoodadKind::Rock,
                "Small Rock",
                2.0,
                0.8,
                1.2,
                None,
                None,
                Some(45.0),
                true,
                DoodadRenderKey::reserved("rock/small"),
            )
            .with_allowed_biomes(forest_biomes())
            .with_spawn_weight(3.0)
            .with_block_radius_meters(1.0),
            DoodadDefinition::new(
                DoodadDefinitionId::new("rock_large"),
                DoodadKind::Rock,
                "Large Rock",
                5.0,
                0.9,
                1.1,
                None,
                None,
                Some(35.0),
                true,
                DoodadRenderKey::reserved("rock/large"),
            )
            .with_allowed_biomes(non_forest_biomes())
            .with_spawn_weight(1.0)
            .with_block_radius_meters(2.5),
            with_random_rotation_y(
                DoodadDefinition::new(
                    DoodadDefinitionId::new("bush_scrub"),
                    DoodadKind::Bush,
                    "Scrub Bush",
                    1.5,
                    0.75,
                    1.25,
                    None,
                    None,
                    Some(30.0),
                    true,
                    DoodadRenderKey::reserved("bush/scrub"),
                )
                .with_allowed_biomes(forest_biomes())
                .with_spawn_weight(5.0),
            ),
            DoodadDefinition::new(
                DoodadDefinitionId::new("ruin_stone"),
                DoodadKind::Ruin,
                "Stone Ruin",
                8.0,
                1.0,
                1.0,
                None,
                None,
                Some(15.0),
                true,
                DoodadRenderKey::reserved("ruin/stone"),
            )
            .with_allowed_biomes(all_assigned_biomes())
            .with_spawn_weight(1.0)
            .with_block_radius_meters(4.0),
            DoodadDefinition::new(
                DoodadDefinitionId::new("resource_node_iron"),
                DoodadKind::ResourceNode,
                "Iron Node",
                3.0,
                1.0,
                1.0,
                None,
                None,
                Some(40.0),
                true,
                DoodadRenderKey::reserved("resource/iron"),
            )
            .with_allowed_biomes(all_assigned_biomes())
            .with_spawn_weight(1.0)
            .with_block_radius_meters(1.5),
        ]
    }
}

#[cfg(test)]
pub use fixtures::starter_definitions;

#[cfg(not(test))]
pub fn starter_definitions() -> Vec<crate::world::doodad::catalog::definition::DoodadDefinition> {
    Vec::new()
}