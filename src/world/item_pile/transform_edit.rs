//! Authoritative world item pile placement editing (yaw + position).

use bevy::prelude::*;

use super::id::ItemPileId;
use super::record::WorldItemPileRecord;
use super::store::ItemPileStore;
use crate::world::{ChunkId, WorldData, WorldPosition};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemPileTransformCandidate {
    pub position: WorldPosition,
    pub yaw_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemPileTransformEditReport {
    pub pile_id: ItemPileId,
    pub previous: ItemPileTransformCandidate,
    pub new: ItemPileTransformCandidate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemPileTransformEditError {
    PileNotFound(ItemPileId),
    InvalidYaw,
    Store(super::error::ItemPileError),
}

impl From<super::error::ItemPileError> for ItemPileTransformEditError {
    fn from(value: super::error::ItemPileError) -> Self {
        Self::Store(value)
    }
}

pub fn update_item_pile_transform(
    world: &mut WorldData,
    pile_id: ItemPileId,
    candidate: ItemPileTransformCandidate,
) -> Result<ItemPileTransformEditReport, ItemPileTransformEditError> {
    if !candidate.yaw_degrees.is_finite() {
        return Err(ItemPileTransformEditError::InvalidYaw);
    }
    let previous_record = world
        .item_pile_store()
        .get(pile_id)
        .cloned()
        .ok_or(ItemPileTransformEditError::PileNotFound(pile_id))?;
    let previous = ItemPileTransformCandidate {
        position: previous_record.placement,
        yaw_degrees: previous_record.yaw_degrees,
    };
    update_item_pile_placement(
        world.item_pile_store_mut(),
        pile_id,
        candidate.position,
        candidate.yaw_degrees,
    )?;
    Ok(ItemPileTransformEditReport {
        pile_id,
        previous,
        new: candidate,
    })
}

pub fn update_item_pile_placement(
    store: &mut ItemPileStore,
    pile_id: ItemPileId,
    placement: WorldPosition,
    yaw_degrees: f32,
) -> Result<(), super::error::ItemPileError> {
    let old_chunk = store
        .pile_chunk(pile_id)
        .ok_or(super::error::ItemPileError::ItemPileNotFound(pile_id))?;
    let record = store
        .get(pile_id)
        .cloned()
        .ok_or(super::error::ItemPileError::ItemPileNotFound(pile_id))?;
    let mut updated = record;
    updated.placement = placement;
    updated.yaw_degrees = yaw_degrees;
    let new_chunk = ChunkId::new(placement.chunk);
    if old_chunk == new_chunk {
        let entry = store.get_mut(pile_id).expect("chunk unchanged");
        entry.placement = placement;
        entry.yaw_degrees = yaw_degrees;
    } else {
        store.remove(pile_id);
        store.insert(new_chunk, updated)?;
    }
    Ok(())
}

impl WorldItemPileRecord {
    pub fn rotation_quat(&self) -> Quat {
        Quat::from_rotation_y(self.yaw_degrees.to_radians())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, ChunkCoord, ChunkData, ChunkLayout, Heightfield, ItemDefinitionId,
        ItemPileSource, LocalPosition, SpaceId,
    };
    use bevy::prelude::Vec3;

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

    fn spawn_pile(world: &mut WorldData) -> ItemPileId {
        let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
        let record = WorldItemPileRecord::new_stack(
            pile_id,
            WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO)),
            SpaceId::SURFACE,
            ItemDefinitionId::new("gold"),
            1,
            None,
            None,
            Affiliation::Player,
            ItemPileSource::DevSpawned,
            0,
        );
        world
            .item_pile_store_mut()
            .insert(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();
        pile_id
    }

    #[test]
    fn default_yaw_is_zero() {
        let mut world = flat_world();
        let pile_id = spawn_pile(&mut world);
        let record = world.item_pile_store().get(pile_id).unwrap();
        assert_eq!(record.yaw_degrees, 0.0);
        assert_eq!(record.rotation_quat(), Quat::IDENTITY);
    }

    #[test]
    fn update_yaw_persists() {
        let mut world = flat_world();
        let pile_id = spawn_pile(&mut world);
        let placement = world.item_pile_store().get(pile_id).unwrap().placement;
        update_item_pile_transform(
            &mut world,
            pile_id,
            ItemPileTransformCandidate {
                position: placement,
                yaw_degrees: 90.0,
            },
        )
        .unwrap();
        assert_eq!(world.item_pile_store().get(pile_id).unwrap().yaw_degrees, 90.0);
    }
}
