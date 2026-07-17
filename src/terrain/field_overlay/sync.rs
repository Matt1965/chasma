//! Sync terrain field overlay meshes with streaming and selection (ADR-103).

use bevy::prelude::*;

use crate::world::{TerrainFieldCatalog, TerrainFieldId, WorldData};

use super::components::TerrainFieldOverlayMesh;
use super::mesh::{TerrainFieldOverlayAssets, build_field_overlay_mesh};
use super::state::TerrainOverlayState;
use crate::terrain::components::TerrainChunkMesh;
use crate::terrain::spawn::TerrainRenderAssets;

/// Diagnostics for Dev overlay inspection (ADR-103).
#[derive(Resource, Debug, Clone, Default)]
pub struct TerrainFieldOverlayDiagnostics {
    pub resident_overlays: usize,
    pub uploads: u64,
    pub cache_hits: u64,
    pub missing_tiles: u64,
    pub last_request_revision: u64,
    pub active_field: Option<TerrainFieldId>,
}

pub fn sync_terrain_field_overlays(
    overlay_state: Res<TerrainOverlayState>,
    catalog: Res<TerrainFieldCatalog>,
    world: Res<WorldData>,
    render_assets: Res<TerrainRenderAssets>,
    overlay_assets: Res<TerrainFieldOverlayAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    terrain_chunks: Query<(Entity, &TerrainChunkMesh)>,
    overlays: Query<(Entity, &TerrainFieldOverlayMesh)>,
    mut diagnostics: ResMut<TerrainFieldOverlayDiagnostics>,
) {
    diagnostics.resident_overlays = 0;
    diagnostics.last_request_revision = overlay_state.request_revision;
    diagnostics.active_field = overlay_state.effective_field().cloned();

    let Some(field_id) = overlay_state.effective_field() else {
        for (entity, _) in &overlays {
            commands.entity(entity).despawn();
        }
        return;
    };

    let Some(definition) = catalog.get(field_id) else {
        for (entity, _) in &overlays {
            commands.entity(entity).despawn();
        }
        return;
    };
    if !definition.enabled || !definition.overlay_style.enabled {
        for (entity, _) in &overlays {
            commands.entity(entity).despawn();
        }
        return;
    }

    let style = &definition.overlay_style;
    let opacity_bp = overlay_state.opacity_basis_points;
    let revision = overlay_state.request_revision;
    let vertical_scale = render_assets.vertical_scale;

    let mut expected = std::collections::HashSet::new();
    for (terrain_entity, marker) in &terrain_chunks {
        let chunk_id = marker.chunk;
        expected.insert(chunk_id);

        let existing = overlays
            .iter()
            .find(|(_, overlay)| overlay.chunk == chunk_id);
        let tile = world.terrain_fields().get_tile(field_id, chunk_id.coord());
        let tile_revision = tile.map(|t| t.tile_revision).unwrap_or(0);

        let needs_rebuild = match existing {
            Some((_, overlay)) => {
                overlay.field_id != *field_id
                    || overlay.request_revision != revision
                    || overlay.tile_revision != tile_revision
            }
            None => true,
        };

        if !needs_rebuild {
            diagnostics.cache_hits += 1;
            diagnostics.resident_overlays += 1;
            continue;
        }

        if let Some((entity, _)) = existing {
            commands.entity(entity).despawn();
        }

        let Some(chunk_data) = world.get(chunk_id) else {
            continue;
        };

        if tile.is_none() {
            diagnostics.missing_tiles += 1;
        }

        let mesh = build_field_overlay_mesh(
            &chunk_data.heightfield,
            tile,
            style,
            opacity_bp,
            vertical_scale,
        );
        let mesh_handle = meshes.add(mesh);
        diagnostics.uploads += 1;
        diagnostics.resident_overlays += 1;

        commands.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(overlay_assets.material.clone()),
            Transform::from_xyz(
                chunk_id.coord().x as f32 * world.layout().chunk_size_units(),
                0.0,
                chunk_id.coord().z as f32 * world.layout().chunk_size_units(),
            ),
            TerrainFieldOverlayMesh {
                chunk: chunk_id,
                field_id: field_id.clone(),
                request_revision: revision,
                tile_revision,
            },
        ));
        let _ = terrain_entity;
    }

    for (entity, overlay) in &overlays {
        if !expected.contains(&overlay.chunk) {
            commands.entity(entity).despawn();
        }
    }
}

/// Despawn overlay meshes when terrain chunks unload.
pub fn despawn_field_overlays_for_chunk(
    commands: &mut Commands,
    chunk_id: crate::world::ChunkId,
    overlays: &Query<(Entity, &TerrainFieldOverlayMesh)>,
) {
    for (entity, marker) in overlays {
        if marker.chunk == chunk_id {
            commands.entity(entity).despawn();
        }
    }
}

pub fn despawn_all_field_overlays(
    commands: &mut Commands,
    overlays: &Query<(Entity, &TerrainFieldOverlayMesh)>,
) {
    for (entity, _) in overlays {
        commands.entity(entity).despawn();
    }
}

/// Mirror terrain unload: remove overlay entities for unloaded chunks.
pub fn cleanup_orphan_field_overlays(
    mut commands: Commands,
    terrain_chunks: Query<&TerrainChunkMesh>,
    overlays: Query<(Entity, &TerrainFieldOverlayMesh)>,
) {
    let resident: std::collections::HashSet<_> = terrain_chunks.iter().map(|m| m.chunk).collect();
    for (entity, overlay) in &overlays {
        if !resident.contains(&overlay.chunk) {
            commands.entity(entity).despawn();
        }
    }
}
