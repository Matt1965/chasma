//! Authoritative world item pile placement editing (orientation + position).

use bevy::prelude::*;

use super::id::ItemPileId;
use super::record::WorldItemPileRecord;
use super::store::ItemPileStore;
use crate::world::authoring_transform::QuantizedOrientation;
use crate::world::{ChunkId, WorldData, WorldPosition};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemPileTransformCandidate {
    pub position: WorldPosition,
    pub orientation: QuantizedOrientation,
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
    InvalidOrientation,
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
    if QuantizedOrientation::from_quat(candidate.orientation.to_quat()).is_err() {
        return Err(ItemPileTransformEditError::InvalidOrientation);
    }
    let previous_record = world
        .item_pile_store()
        .get(pile_id)
        .cloned()
        .ok_or(ItemPileTransformEditError::PileNotFound(pile_id))?;
    let previous = ItemPileTransformCandidate {
        position: previous_record.placement,
        orientation: previous_record.orientation,
    };
    update_item_pile_placement(
        world.item_pile_store_mut(),
        pile_id,
        candidate.position,
        candidate.orientation,
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
    orientation: QuantizedOrientation,
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
    updated.orientation = orientation;
    let new_chunk = ChunkId::new(placement.chunk);
    if old_chunk == new_chunk {
        let entry = store.get_mut(pile_id).expect("chunk unchanged");
        entry.placement = placement;
        entry.orientation = orientation;
    } else {
        store.remove(pile_id);
        store.insert(new_chunk, updated)?;
    }
    Ok(())
}

impl WorldItemPileRecord {
    pub fn rotation_quat(&self) -> Quat {
        self.orientation.to_quat()
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
    fn default_orientation_is_identity() {
        let mut world = flat_world();
        let pile_id = spawn_pile(&mut world);
        let record = world.item_pile_store().get(pile_id).unwrap();
        assert_eq!(record.orientation, QuantizedOrientation::IDENTITY);
        assert_eq!(record.rotation_quat(), Quat::IDENTITY);
    }

    #[test]
    fn update_orientation_persists() {
        let mut world = flat_world();
        let pile_id = spawn_pile(&mut world);
        let placement = world.item_pile_store().get(pile_id).unwrap().placement;
        let orientation = QuantizedOrientation::from_degrees(90.0, 0.0, 0.0).unwrap();
        update_item_pile_transform(
            &mut world,
            pile_id,
            ItemPileTransformCandidate {
                position: placement,
                orientation,
            },
        )
        .unwrap();
        assert_eq!(
            world.item_pile_store().get(pile_id).unwrap().orientation,
            orientation
        );
    }
}
