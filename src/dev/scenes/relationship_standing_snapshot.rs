//! Scene persistence for relationship Standing (ADR-132 Phase 3).

use serde::{Deserialize, Serialize};

use crate::world::{RelationshipStandingSaveState, WorldData};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneRelationshipStandingPersistence {
    #[serde(default)]
    pub save_state: RelationshipStandingSaveState,
}

pub fn capture_relationship_standing_persistence(
    world: &WorldData,
) -> SceneRelationshipStandingPersistence {
    SceneRelationshipStandingPersistence {
        save_state: world.relationship_standing_store().export_save_state(),
    }
}

pub fn restore_relationship_standing_persistence(
    world: &mut WorldData,
    persistence: &SceneRelationshipStandingPersistence,
) {
    world
        .relationship_standing_store_mut()
        .import_save_state(persistence.save_state.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::scenes::snapshot::SceneDefinition;
    use crate::dev::scenes::{SceneCaptureContext, apply_scene, capture_scene};
    use crate::world::relationship::{
        FactionId, RelationshipFacet, RelationshipStandingSaveState, RelationshipStandingStore,
        SpeciesId,
    };
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, UnitCatalog, UnitDefinitionId,
        UnitId, UnitSource, WorldData, WorldPosition, create_unit,
    };
    use bevy::prelude::Vec3;

    use crate::world::LocalPosition;

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

    fn sample_scene(world: &WorldData) -> SceneDefinition {
        let ctx = SceneCaptureContext {
            name: "standing".into(),
            description: String::new(),
            tags: Vec::new(),
            created_at: 1,
            world_seed: 0,
            camera_state: None,
            debug_flags: None,
        };
        capture_scene(world, &ctx)
    }

    #[test]
    fn standing_scene_round_trip_preserves_edges() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let unit_a = create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Dev,
        )
        .unwrap()
        .id;
        let unit_b = create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("deer"),
            pos(2.0, 2.0),
            UnitSource::Dev,
        )
        .unwrap()
        .id;
        world.relationship_standing_store_mut().apply_delta(
            RelationshipFacet::Faction(FactionId::new("wild")),
            RelationshipFacet::Species(SpeciesId::new("deer")),
            12,
        );
        world.relationship_standing_store_mut().apply_delta(
            RelationshipFacet::Species(SpeciesId::new("wolf")),
            RelationshipFacet::Species(SpeciesId::new("deer")),
            -8,
        );
        world.relationship_standing_store_mut().apply_delta(
            RelationshipFacet::Individual(unit_a),
            RelationshipFacet::Individual(unit_b),
            150,
        );

        let scene = sample_scene(&world);
        apply_scene(
            &mut world,
            &unit_catalog,
            &crate::world::DoodadCatalog::default(),
            &crate::world::BuildingCatalog::default(),
            &crate::world::FootprintCatalog::default(),
            &crate::world::InteriorProfileCatalog::default(),
            None,
            &scene,
        )
        .unwrap();

        assert_eq!(
            world.relationship_standing_store().get(
                &RelationshipFacet::Faction(FactionId::new("wild")),
                &RelationshipFacet::Species(SpeciesId::new("deer")),
            ),
            12
        );
        assert_eq!(
            world.relationship_standing_store().get(
                &RelationshipFacet::Species(SpeciesId::new("wolf")),
                &RelationshipFacet::Species(SpeciesId::new("deer")),
            ),
            -8
        );
        assert_eq!(
            world.relationship_standing_store().get(
                &RelationshipFacet::Individual(unit_a),
                &RelationshipFacet::Individual(unit_b),
            ),
            150
        );
    }

    #[test]
    fn restored_individual_standing_targets_same_unit_ids() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let unit_a = create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Dev,
        )
        .unwrap()
        .id;
        let unit_b = create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("deer"),
            pos(2.0, 2.0),
            UnitSource::Dev,
        )
        .unwrap()
        .id;
        world.relationship_standing_store_mut().apply_delta(
            RelationshipFacet::Individual(unit_a),
            RelationshipFacet::Individual(unit_b),
            99,
        );
        let scene = sample_scene(&world);
        apply_scene(
            &mut world,
            &unit_catalog,
            &crate::world::DoodadCatalog::default(),
            &crate::world::BuildingCatalog::default(),
            &crate::world::FootprintCatalog::default(),
            &crate::world::InteriorProfileCatalog::default(),
            None,
            &scene,
        )
        .unwrap();
        assert!(world.get_unit(unit_a).is_some());
        assert!(world.get_unit(unit_b).is_some());
        assert_eq!(
            world.relationship_standing_store().get(
                &RelationshipFacet::Individual(unit_a),
                &RelationshipFacet::Individual(unit_b),
            ),
            99
        );
    }

    #[test]
    fn scene_without_standing_field_restores_successfully() {
        let state: RelationshipStandingSaveState = Default::default();
        let mut store = RelationshipStandingStore::default();
        store.import_save_state(state);
        assert!(store.is_empty());
    }
}
