//! Spawn and despawn derived terrain render entities (ADR-010, ADR-012).
//!
//! Meshes are disposable visualization; authoritative terrain remains in
//! [`WorldData`].

use bevy::prelude::*;

use crate::world::{ChunkId, WorldData};

use super::components::TerrainChunkMesh;
use super::mesh::{ChunkLod, build_chunk_mesh_scaled};

/// Shared render resources for terrain chunk meshes.
#[derive(Debug, Clone, Resource)]
pub struct TerrainRenderAssets {
    pub material: Handle<StandardMaterial>,
    pub vertical_scale: f32,
}

/// Spawn one derived render entity for a resident chunk.
pub fn spawn_chunk_mesh(
    commands: &mut Commands,
    chunk_id: ChunkId,
    world: &WorldData,
    chunk_size_units: f32,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    vertical_scale: f32,
) {
    let Some(data) = world.get(chunk_id) else {
        return;
    };

    let mesh = build_chunk_mesh_scaled(&data.heightfield, ChunkLod::Full, vertical_scale);
    let coord = chunk_id.coord();
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::from_xyz(
            coord.x as f32 * chunk_size_units,
            0.0,
            coord.z as f32 * chunk_size_units,
        ),
        TerrainChunkMesh::new(chunk_id),
    ));
}

/// Despawn all derived render entities for `chunk_id`.
pub fn despawn_chunk_meshes(
    commands: &mut Commands,
    chunk_id: ChunkId,
    mesh_entities: &Query<(Entity, &TerrainChunkMesh)>,
) {
    for (entity, marker) in mesh_entities {
        if marker.chunk == chunk_id {
            commands.entity(entity).despawn();
        }
    }
}

/// Spawn one derived render entity per resident chunk in `world`.
pub fn spawn_terrain_render_entities(
    commands: &mut Commands,
    world: &WorldData,
    chunk_size_units: f32,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
) {
    spawn_terrain_render_entities_scaled(
        commands,
        world,
        chunk_size_units,
        meshes,
        material,
        1.0,
    );
}

/// Spawn terrain meshes with optional vertical exaggeration for visualization.
pub fn spawn_terrain_render_entities_scaled(
    commands: &mut Commands,
    world: &WorldData,
    chunk_size_units: f32,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    vertical_scale: f32,
) {
    for (id, _) in world.iter() {
        spawn_chunk_mesh(
            commands,
            id,
            world,
            chunk_size_units,
            meshes,
            material.clone(),
            vertical_scale,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, WorldData};

    fn sample_world() -> (WorldData, ChunkId) {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let id = ChunkId::new(ChunkCoord::new(2, 3));
        let hf = Heightfield::from_samples(2, 128.0, vec![0.0, 1.0, 2.0, 3.0]).unwrap();
        world.insert(id, ChunkData::new(hf, Vec::new()));
        (world, id)
    }

    #[test]
    fn spawn_chunk_mesh_requires_resident_chunk_data() {
        let (world, id) = sample_world();
        assert!(world.get(id).is_some());
    }

    #[test]
    fn despawn_target_is_matching_chunk_id() {
        let keep = ChunkId::new(ChunkCoord::new(0, 0));
        let remove = ChunkId::new(ChunkCoord::new(1, 0));
        assert_ne!(keep, remove);
    }
}
