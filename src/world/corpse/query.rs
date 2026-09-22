use super::id::CorpseId;
use super::record::{CorpseRecord, CorpseState};
use super::settings::CorpseSettings;
use crate::world::quantized_distance_squared_cm;
use crate::world::{ChunkCoord, ChunkId, SpaceId, WorldData, WorldPosition};

/// Nearest present corpse at a world position within interaction radius (3×3 chunk scan).
pub fn nearest_corpse_at_position<'a>(
    world: &'a WorldData,
    position: WorldPosition,
    space_id: SpaceId,
    settings: &CorpseSettings,
) -> Option<&'a CorpseRecord> {
    let max_dist_sq = settings.interaction_radius_squared_cm();
    let mut best: Option<(i64, CorpseId, &'a CorpseRecord)> = None;

    let mut chunks: Vec<ChunkCoord> = Vec::with_capacity(9);
    for dz in -1..=1 {
        for dx in -1..=1 {
            chunks.push(ChunkCoord::new(
                position.chunk.x + dx,
                position.chunk.z + dz,
            ));
        }
    }
    chunks.sort_by_key(|coord| (coord.x, coord.z));

    for chunk_coord in chunks {
        let chunk_id = ChunkId::new(chunk_coord);
        for corpse in world.corpse_store().corpses_in_chunk(chunk_id) {
            if corpse.state != CorpseState::Present {
                continue;
            }
            if corpse.current_space_id != space_id {
                continue;
            }
            let dist = quantized_distance_squared_cm(position, corpse.placement.position);
            if dist > max_dist_sq {
                continue;
            }
            let replace = match &best {
                None => true,
                Some((best_dist, best_id, _)) => {
                    dist < *best_dist - 1 || (dist - *best_dist).abs() <= 1 && corpse.id < *best_id
                }
            };
            if replace {
                best = Some((dist, corpse.id, corpse));
            }
        }
    }

    best.map(|(_, _, corpse)| corpse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        UnitDefinitionId, UnitPlacement, UnitSource, create_unit_with_inventory,
        starter_inventory_profile_definitions, starter_item_category_definitions,
        starter_item_definitions, starter_unit_definitions,
    };
    use crate::world::{
        InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, UnitCatalog,
        UnitOwnership,
    };
    use bevy::prelude::{Quat, Vec3};

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
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

    fn test_ctx() -> InventoryCatalogCtx<'static> {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items =
            ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    }

    fn insert_corpse(world: &mut WorldData, id: u64, position: WorldPosition) -> CorpseId {
        let corpse_id = CorpseId::new(id);
        let record = CorpseRecord::new(
            corpse_id,
            crate::world::UnitId::new(1),
            UnitDefinitionId::new("bandit"),
            UnitPlacement::new(position, Quat::IDENTITY),
            SpaceId::SURFACE,
            None,
            None,
            None,
            None,
            None,
            Affiliation::Unknown,
            0,
            100,
        );
        let chunk = ChunkId::new(position.chunk);
        world.corpse_store_mut().insert(chunk, record).unwrap();
        corpse_id
    }

    #[test]
    fn nearest_corpse_prefers_closest_deterministic_tie_break() {
        let mut world = flat_world();
        let far_id = insert_corpse(&mut world, 1, pos(40.0, 40.0));
        let near_id = insert_corpse(&mut world, 2, pos(40.5, 40.5));
        let settings = CorpseSettings::default();
        let found = nearest_corpse_at_position(&world, pos(40.0, 40.0), SpaceId::SURFACE, &settings)
            .expect("corpse");
        assert_eq!(found.id, far_id);
        assert_ne!(found.id, near_id);
    }

    #[test]
    fn expired_corpse_is_ignored() {
        let mut world = flat_world();
        let corpse_id = insert_corpse(&mut world, 1, pos(10.0, 10.0));
        world
            .corpse_store_mut()
            .get_mut(corpse_id)
            .unwrap()
            .state = CorpseState::Expired;
        assert!(
            nearest_corpse_at_position(
                &world,
                pos(10.0, 10.0),
                SpaceId::SURFACE,
                &CorpseSettings::default(),
            )
            .is_none()
        );
    }
}
