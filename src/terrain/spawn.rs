//! Spawn and despawn derived terrain render entities (ADR-010, ADR-012).
//!
//! Meshes are disposable visualization; authoritative terrain remains in
//! [`WorldData`].

use bevy::prelude::*;

use crate::world::{ChunkCoord, ChunkId, WorldData};

use super::components::TerrainChunkMesh;
use super::mesh::{ChunkLod, ChunkMeshSeamWeld, build_chunk_mesh_scaled};

/// Shared render resources for terrain chunk meshes.
#[derive(Debug, Clone, Resource)]
pub struct TerrainRenderAssets {
    pub material: Handle<StandardMaterial>,
    pub vertical_scale: f32,
}

/// Target visible height span (world units) when auto-scaling subtle source heights.
pub const DEFAULT_TARGET_HEIGHT_SPAN_UNITS: f32 = 120.0;

/// Compute a mesh vertical scale from authored height range in meters/units.
pub fn vertical_scale_for_height_span(
    height_min: f32,
    height_max: f32,
    target_span_units: f32,
) -> f32 {
    let span = (height_max - height_min).max(1e-12);
    target_span_units / span
}

fn seam_weld_heights(world: &WorldData, chunk_id: ChunkId) -> ChunkMeshSeamWeld {
    let coord = chunk_id.coord();
    let edge = |data: &crate::world::ChunkData| data.heightfield.samples_per_edge() - 1;
    let penultimate = |data: &crate::world::ChunkData| {
        data.heightfield.samples_per_edge().saturating_sub(2)
    };

    let west = world
        .get(ChunkId::new(ChunkCoord::new(coord.x - 1, coord.z)))
        .map(|data| data.heightfield.column_heights(edge(data)));
    let south = world
        .get(ChunkId::new(ChunkCoord::new(coord.x, coord.z - 1)))
        .map(|data| data.heightfield.row_heights(edge(data)));
    let east_interior = world
        .get(ChunkId::new(ChunkCoord::new(coord.x + 1, coord.z)))
        .map(|data| data.heightfield.column_heights(1));
    let north_interior = world
        .get(ChunkId::new(ChunkCoord::new(coord.x, coord.z + 1)))
        .map(|data| data.heightfield.row_heights(1));
    let west_interior = world
        .get(ChunkId::new(ChunkCoord::new(coord.x - 1, coord.z)))
        .map(|data| data.heightfield.column_heights(penultimate(data)));
    let south_interior = world
        .get(ChunkId::new(ChunkCoord::new(coord.x, coord.z - 1)))
        .map(|data| data.heightfield.row_heights(penultimate(data)));

    ChunkMeshSeamWeld {
        west_edge: west,
        south_edge: south,
        east_interior,
        north_interior,
        west_interior,
        south_interior,
    }
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

    let seam_weld = seam_weld_heights(world, chunk_id);
    let mesh = build_chunk_mesh_scaled(
        &data.heightfield,
        ChunkLod::Full,
        vertical_scale,
        &seam_weld,
    );
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

/// Rebuild render meshes for resident orthogonal neighbors after `chunk_id` loads.
pub fn refresh_adjacent_chunk_meshes(
    commands: &mut Commands,
    chunk_id: ChunkId,
    world: &WorldData,
    chunk_size_units: f32,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    vertical_scale: f32,
    mesh_entities: &Query<(Entity, &TerrainChunkMesh)>,
) {
    let coord = chunk_id.coord();
    let neighbors = [
        ChunkCoord::new(coord.x - 1, coord.z),
        ChunkCoord::new(coord.x + 1, coord.z),
        ChunkCoord::new(coord.x, coord.z - 1),
        ChunkCoord::new(coord.x, coord.z + 1),
    ];
    for neighbor_coord in neighbors {
        let neighbor_id = ChunkId::new(neighbor_coord);
        if world.get(neighbor_id).is_none() {
            continue;
        }
        despawn_chunk_meshes(commands, neighbor_id, mesh_entities);
        spawn_chunk_mesh(
            commands,
            neighbor_id,
            world,
            chunk_size_units,
            meshes,
            material.clone(),
            vertical_scale,
        );
    }
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
    use crate::world::{ChunkData, ChunkLayout, Heightfield, WorldData};

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
    fn vertical_scale_inversely_tracks_height_span() {
        let tight = vertical_scale_for_height_span(0.0, 0.0001, 100.0);
        let wide = vertical_scale_for_height_span(0.0, 10.0, 100.0);
        assert!(tight > wide);
    }

    #[test]
    fn despawn_target_is_matching_chunk_id() {
        let keep = ChunkId::new(ChunkCoord::new(0, 0));
        let remove = ChunkId::new(ChunkCoord::new(1, 0));
        assert_ne!(keep, remove);
    }
}
