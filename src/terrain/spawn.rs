//! Spawn derived terrain render entities from authoritative [`WorldData`] (ADR-010).
//!
//! This is the reusable bridge between loaded world data and visible terrain.
//! It owns no authoritative state: it reads [`WorldData`], builds disposable
//! meshes, and spawns render entities marked with [`TerrainChunkMesh`].

use bevy::prelude::*;

use crate::world::WorldData;

use super::components::TerrainChunkMesh;
use super::mesh::{ChunkLod, build_chunk_mesh_scaled};

/// Spawn one derived render entity per loaded chunk in `world`.
///
/// Each entity receives a full-resolution mesh, the shared `material`, a
/// `Transform` at the chunk minimum corner, and a [`TerrainChunkMesh`] marker.
/// Authoritative terrain remains in [`WorldData`]; these entities are
/// disposable visualization only (ADR-010, ADR-013).
///
/// Callers must ensure this is not invoked twice for the same chunks unless
/// duplicates are intentional. Phase 2A has no unload path.
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
    for (id, data) in world.iter() {
        let mesh = build_chunk_mesh_scaled(&data.heightfield, ChunkLod::Full, vertical_scale);
        let coord = id.coord();
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(
                coord.x as f32 * chunk_size_units,
                0.0,
                coord.z as f32 * chunk_size_units,
            ),
            TerrainChunkMesh::new(id),
        ));
    }
}
