//! Filled Navigation/Pathing Mask meshes (dev presentation).
//!
//! Mirrors terrain-field overlay style: unlit vertex-colored quads above terrain.
//! Observes cached passability samples only — does not alter navigation data.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::debug::settings::DebugOverlaySettings;
use crate::terrain::TerrainRenderAssets;
use crate::world::{ChunkLayout, WorldConfig, WorldData};

use super::nav_cells::sample_terrain_y;
use super::navigation_overlay::{
    NavigationMaskCache, NavigationMaskCellSample, NavigationMaskDrawStats, block_reason_color,
};

const MASK_Y_LIFT: f32 = 0.35;
const WALKABLE_COLOR: [f32; 4] = [0.1, 0.92, 0.35, 0.48];

/// Marker for spawned pathing-mask mesh entities (never serialized).
#[derive(Component, Debug)]
pub struct NavigationMaskOverlayMesh {
    pub cache_revision: u64,
    pub draw_walkable: bool,
    pub draw_blockers: bool,
}

/// Shared unlit translucent material for the pathing mask.
#[derive(Resource, Debug, Clone)]
pub struct NavigationMaskOverlayAssets {
    pub material: Handle<StandardMaterial>,
}

pub fn setup_navigation_mask_overlay_assets(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(NavigationMaskOverlayAssets { material });
}

/// Spawn/rebuild/despawn filled mask meshes from [`NavigationMaskCache`].
pub fn sync_navigation_mask_meshes(
    settings: Res<DebugOverlaySettings>,
    cache: Res<NavigationMaskCache>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    overlay_assets: Option<Res<NavigationMaskOverlayAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    existing: Query<(Entity, &NavigationMaskOverlayMesh)>,
    mut stats: ResMut<NavigationMaskDrawStats>,
) {
    let draw_walkable = settings.enabled && settings.grid;
    let draw_blockers = settings.enabled && (settings.grid || settings.nav_blockers);
    let mask_on = draw_walkable || draw_blockers;

    if !mask_on || cache.samples.is_empty() {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        if !mask_on {
            // Keep sample stats from the draw system; clear draw counts when hidden.
            stats.navigable_drawn = 0;
            stats.blocked_drawn = 0;
        }
        return;
    }

    let Some(overlay_assets) = overlay_assets else {
        return;
    };

    let needs_rebuild = existing.iter().next().map_or(true, |(_, marker)| {
        marker.cache_revision != cache.revision
            || marker.draw_walkable != draw_walkable
            || marker.draw_blockers != draw_blockers
    });

    if !needs_rebuild {
        return;
    }

    for (entity, _) in &existing {
        commands.entity(entity).despawn();
    }

    let layout = world.layout();
    let _ = config;
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let half = if cache.cell_spacing_meters > 0.01 {
        cache.cell_spacing_meters * 0.42
    } else {
        estimate_half_extent(&cache.samples).unwrap_or(1.68)
    };

    let (mesh, navigable, blocked) = build_navigation_mask_mesh(
        &cache.samples,
        &world,
        layout,
        vertical_scale,
        half,
        draw_walkable,
        draw_blockers,
    );
    stats.navigable_drawn = navigable;
    stats.blocked_drawn = blocked;
    stats.cells_sampled = cache.samples.len() as u32;
    stats.ran = true;

    if navigable + blocked == 0 {
        return;
    }

    let handle = meshes.add(mesh);
    commands.spawn((
        Mesh3d(handle),
        MeshMaterial3d(overlay_assets.material.clone()),
        Transform::IDENTITY,
        NavigationMaskOverlayMesh {
            cache_revision: cache.revision,
            draw_walkable,
            draw_blockers,
        },
    ));
}

fn estimate_half_extent(samples: &[NavigationMaskCellSample]) -> Option<f32> {
    let a = samples.first()?;
    let b = samples
        .iter()
        .find(|s| s.coord.z == a.coord.z && s.coord.x != a.coord.x)?;
    let spacing = (b.center_xz.x - a.center_xz.x).abs();
    if spacing > 0.01 {
        Some(spacing * 0.42)
    } else {
        None
    }
}

/// Build a single triangle-list mesh of colored quads for mask cells.
pub fn build_navigation_mask_mesh(
    samples: &[NavigationMaskCellSample],
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    half_extent: f32,
    draw_walkable: bool,
    draw_blockers: bool,
) -> (Mesh, u32, u32) {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut navigable = 0_u32;
    let mut blocked = 0_u32;

    for sample in samples {
        let rgba = if sample.walkable {
            if !draw_walkable {
                continue;
            }
            navigable += 1;
            WALKABLE_COLOR
        } else {
            if !draw_blockers {
                continue;
            }
            blocked += 1;
            let c = sample
                .block_reason
                .map(block_reason_color)
                .unwrap_or(Color::srgba(0.3, 0.3, 0.3, 0.55));
            let s = c.to_srgba();
            [s.red, s.green, s.blue, s.alpha.min(0.55)]
        };

        let y = sample_terrain_y(world, sample.center_xz, layout, vertical_scale) + MASK_Y_LIFT;
        let x = sample.center_xz.x;
        let z = sample.center_xz.y;
        let base = positions.len() as u32;
        positions.push([x - half_extent, y, z - half_extent]);
        positions.push([x + half_extent, y, z - half_extent]);
        positions.push([x + half_extent, y, z + half_extent]);
        positions.push([x - half_extent, y, z + half_extent]);
        colors.extend_from_slice(&[rgba, rgba, rgba, rgba]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    (mesh, navigable, blocked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, GridCoord, Heightfield, PassabilityBlockReason,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    #[test]
    fn mask_mesh_emits_quads_for_navigable_and_blocked() {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let samples = vec![
            NavigationMaskCellSample {
                coord: GridCoord::new(0, 0),
                center_xz: Vec2::new(2.0, 2.0),
                walkable: true,
                block_reason: None,
            },
            NavigationMaskCellSample {
                coord: GridCoord::new(1, 0),
                center_xz: Vec2::new(6.0, 2.0),
                walkable: false,
                block_reason: Some(PassabilityBlockReason::BuildingOccupied),
            },
        ];
        let (mesh, nav, blocked) =
            build_navigation_mask_mesh(&samples, &world, layout(), 1.0, 1.5, true, true);
        assert_eq!(nav, 1);
        assert_eq!(blocked, 1);
        assert_eq!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len(), 8);
    }

    #[test]
    fn mask_mesh_empty_when_toggles_draw_nothing() {
        let world = WorldData::new(layout());
        let samples = vec![NavigationMaskCellSample {
            coord: GridCoord::new(0, 0),
            center_xz: Vec2::new(2.0, 2.0),
            walkable: true,
            block_reason: None,
        }];
        let (mesh, nav, blocked) =
            build_navigation_mask_mesh(&samples, &world, layout(), 1.0, 1.5, false, false);
        assert_eq!(nav, 0);
        assert_eq!(blocked, 0);
        assert_eq!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len(), 0);
    }
}
