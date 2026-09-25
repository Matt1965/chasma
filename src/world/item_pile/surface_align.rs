//! Terrain-conforming orientation for world item piles.

use bevy::prelude::*;

use super::id::ItemPileId;
use super::transform_edit::{
    ItemPileTransformCandidate, ItemPileTransformEditError, update_item_pile_transform,
};
use crate::world::authoring_transform::QuantizedOrientation;
use crate::world::building::rotation_from_yaw_and_normal;
use crate::world::terrain::estimate_effective_terrain_normal;
use crate::world::{WorldData, WorldPosition};

/// Default pile orientation on spawn: preserve yaw while conforming to terrain.
pub fn default_placement_orientation(
    world: &WorldData,
    position: WorldPosition,
    yaw_degrees: f32,
) -> QuantizedOrientation {
    align_orientation_yaw_to_surface(
        world,
        position,
        QuantizedOrientation::from_degrees(yaw_degrees, 0.0, 0.0).unwrap_or(QuantizedOrientation::IDENTITY),
    )
}

/// Realign to terrain using yaw only; pitch and roll come from the surface normal.
pub fn align_orientation_yaw_to_surface(
    world: &WorldData,
    position: WorldPosition,
    orientation: QuantizedOrientation,
) -> QuantizedOrientation {
    let normal = estimate_effective_terrain_normal(world, position).unwrap_or(Vec3::Y);
    let quat = rotation_from_yaw_and_normal(orientation.yaw_degrees().to_radians(), normal);
    QuantizedOrientation::from_quat(quat).unwrap_or(orientation)
}

/// Realign to terrain while preserving authored pitch and roll offsets.
pub fn align_orientation_to_surface(
    world: &WorldData,
    position: WorldPosition,
    orientation: QuantizedOrientation,
) -> QuantizedOrientation {
    let yaw = orientation.yaw_degrees();
    let pitch = orientation.pitch_degrees();
    let roll = orientation.roll_degrees();
    let normal = estimate_effective_terrain_normal(world, position).unwrap_or(Vec3::Y);
    let base = rotation_from_yaw_and_normal(yaw.to_radians(), normal);
    let local = Quat::from_euler(EulerRot::YXZ, 0.0, pitch.to_radians(), roll.to_radians());
    QuantizedOrientation::from_quat(base * local).unwrap_or(orientation)
}

/// Align an authoritative pile record to terrain at its placement.
pub fn align_item_pile_to_surface(
    world: &mut WorldData,
    pile_id: ItemPileId,
) -> Result<QuantizedOrientation, ItemPileTransformEditError> {
    let record = world
        .item_pile_store()
        .get(pile_id)
        .cloned()
        .ok_or(ItemPileTransformEditError::PileNotFound(pile_id))?;
    let aligned = align_orientation_yaw_to_surface(world, record.placement, record.orientation);
    update_item_pile_transform(
        world,
        pile_id,
        ItemPileTransformCandidate {
            position: record.placement,
            orientation: aligned,
        },
    )?;
    Ok(aligned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition,
    };

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn sloped_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let edge = 65usize;
        let mut samples = Vec::with_capacity(edge * edge);
        for _z in 0..edge {
            for x in 0..edge {
                samples.push(x as f32 * 0.2);
            }
        }
        let heightfield = Heightfield::from_samples(65, 4.0, samples).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn sample_position() -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(32.0, 0.0, 32.0)),
        )
    }

    #[test]
    fn flat_terrain_keeps_identity_orientation() {
        let world = flat_world();
        let orientation = default_placement_orientation(&world, sample_position(), 0.0);
        assert_eq!(orientation, QuantizedOrientation::IDENTITY);
    }

    #[test]
    fn sloped_terrain_tilts_pile_toward_normal() {
        let world = sloped_world();
        let orientation = default_placement_orientation(&world, sample_position(), 0.0);
        assert!(
            orientation.pitch_millidegrees != 0 || orientation.roll_millidegrees != 0,
            "expected non-zero tilt on slope, got {orientation:?}"
        );
    }

    #[test]
    fn align_preserves_yaw_on_slope() {
        let world = sloped_world();
        let authored = QuantizedOrientation::from_degrees(45.0, 0.0, 0.0).unwrap();
        let aligned = align_orientation_yaw_to_surface(&world, sample_position(), authored);
        assert!((aligned.yaw_degrees() - 45.0).abs() < 0.5);
    }
}
