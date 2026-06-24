//! Authoritative spawn helpers for dev mode (ADR-043).

use bevy::prelude::*;

use crate::world::{
    create_doodad, create_unit_with_ownership, DoodadCatalog, DoodadDefinitionId,
    DoodadPlacementOverrides, DoodadSource, UnitCatalog, UnitDefinitionId, UnitOwnership,
    UnitSource, WorldData, WorldPosition,
};

use super::dev_mode::{DefinitionId, SpawnMode};

/// Outcome of a dev spawn attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevSpawnOutcome {
    SpawnedUnit { definition_id: UnitDefinitionId },
    SpawnedDoodad { definition_id: DoodadDefinitionId },
    NoDefinitionSelected,
    TerrainMiss,
    AuthoringFailed(String),
}

/// Resolve a terrain raycast hit into a grounded spawn position.
pub fn dev_spawn_position_from_terrain_click(
    world: &WorldData,
    world_position: WorldPosition,
) -> Option<WorldPosition> {
    crate::world::ground_world_position(world, world_position)
}

/// Spawn the selected definition at an authoritative world position via WorldData APIs.
pub fn spawn_selected_at_position(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    selected: Option<&DefinitionId>,
    position: WorldPosition,
    spawn_affiliation: crate::world::Affiliation,
) -> DevSpawnOutcome {
    let Some(definition) = selected else {
        return DevSpawnOutcome::NoDefinitionSelected;
    };

    match definition {
        DefinitionId::Unit(definition_id) => {
            let ownership = UnitOwnership::with_affiliation(spawn_affiliation);
            match create_unit_with_ownership(
                unit_catalog,
                world,
                definition_id,
                position,
                UnitSource::Dev,
                ownership,
            ) {
                Ok(_record) => DevSpawnOutcome::SpawnedUnit {
                    definition_id: definition_id.clone(),
                },
                Err(error) => DevSpawnOutcome::AuthoringFailed(format!("{error:?}")),
            }
        }
        DefinitionId::Doodad(definition_id) => match create_doodad(
            doodad_catalog,
            world,
            definition_id,
            position,
            DoodadSource::Dev,
            DoodadPlacementOverrides::default(),
        ) {
            Ok(_record) => DevSpawnOutcome::SpawnedDoodad {
                definition_id: definition_id.clone(),
            },
            Err(error) => DevSpawnOutcome::AuthoringFailed(format!("{error:?}")),
        },
    }
}

/// Convenience for tests — spawn by explicit mode + id string.
pub fn spawn_by_mode_at_position(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    mode: SpawnMode,
    definition_key: &str,
    position: WorldPosition,
) -> DevSpawnOutcome {
    let selected = match mode {
        SpawnMode::Unit => DefinitionId::Unit(UnitDefinitionId::new(definition_key)),
        SpawnMode::Doodad => DefinitionId::Doodad(DoodadDefinitionId::new(definition_key)),
    };
    spawn_selected_at_position(
        world,
        unit_catalog,
        doodad_catalog,
        Some(&selected),
        position,
        crate::world::Affiliation::Player,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn spawn_uses_world_data_unit_api() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let position = pos(40.0, 40.0);
        let outcome = spawn_by_mode_at_position(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            SpawnMode::Unit,
            "wolf",
            position,
        );
        assert!(matches!(outcome, DevSpawnOutcome::SpawnedUnit { .. }));
        let store = world
            .units_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
            .unwrap();
        assert_eq!(store.len(), 1);
        let record = store.records()[0].clone();
        assert_eq!(record.source, UnitSource::Dev);
    }

    #[test]
    fn spawn_uses_world_data_doodad_api() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let position = pos(50.0, 50.0);
        let outcome = spawn_by_mode_at_position(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            SpawnMode::Doodad,
            "tree_oak",
            position,
        );
        assert!(matches!(outcome, DevSpawnOutcome::SpawnedDoodad { .. }));
        let store = world
            .doodads_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
            .unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(store.records()[0].source, DoodadSource::Dev);
    }

    #[test]
    fn terrain_spawn_position_grounds_click() {
        let world = flat_world();
        let candidate = pos(12.0, 18.0);
        let grounded = dev_spawn_position_from_terrain_click(&world, candidate).unwrap();
        assert_eq!(grounded.chunk, candidate.chunk);
    }
}
