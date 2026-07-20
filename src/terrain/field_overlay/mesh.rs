//! Field overlay mesh generation (ADR-103).
//!
//! Builds a conforming 33×33 vertex grid aligned to authoritative field tiles.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::world::{
    ChunkCoord, Heightfield, LocalPosition, TERRAIN_FIELD_SAMPLE_SPACING_METERS,
    TERRAIN_FIELD_SAMPLES_PER_EDGE, TerrainFieldCatalog, TerrainFieldId, TerrainFieldOverlayStyle,
    WorldData, WorldPosition, bootstrap_constant_field, sample_terrain_field_at,
};

use super::state::TerrainOverlayState;

const OVERLAY_Y_OFFSET: f32 = 0.2;

/// Shared overlay material inserted at startup.
#[derive(Resource, Debug, Clone)]
pub struct TerrainFieldOverlayAssets {
    pub material: Handle<StandardMaterial>,
}

pub fn setup_terrain_field_overlay_assets(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..default()
    });
    commands.insert_resource(TerrainFieldOverlayAssets { material });
}

pub fn build_field_overlay_mesh(
    heightfield: &Heightfield,
    chunk: ChunkCoord,
    field_id: &TerrainFieldId,
    world: &WorldData,
    catalog: &TerrainFieldCatalog,
    style: &TerrainFieldOverlayStyle,
    opacity_bp: u16,
    vertical_scale: f32,
) -> Mesh {
    let samples_per_edge = TERRAIN_FIELD_SAMPLES_PER_EDGE as usize;
    // Overlay vertices sit on the authoritative field grid (8 m spacing), which
    // spans the full chunk. The heightfield is a finer grid (e.g. 257 samples at
    // 1 m), so each field vertex maps to every Nth heightfield sample.
    let spacing = TERRAIN_FIELD_SAMPLE_SPACING_METERS;
    let hf_spacing = heightfield.spacing_meters().max(f32::EPSILON);
    let hf_per_field = (spacing / hf_spacing).round().max(1.0) as usize;
    let mut positions = Vec::with_capacity(samples_per_edge * samples_per_edge);
    let mut colors = Vec::with_capacity(samples_per_edge * samples_per_edge);
    let hf_samples = heightfield.samples_per_edge() as usize;

    for row in 0..samples_per_edge {
        for col in 0..samples_per_edge {
            let local_x = col as f32 * spacing;
            let local_z = row as f32 * spacing;
            let hf_col = (col * hf_per_field).min(hf_samples.saturating_sub(1));
            let hf_row = (row * hf_per_field).min(hf_samples.saturating_sub(1));
            let y = heightfield.samples()[hf_row * hf_samples + hf_col] * vertical_scale
                + OVERLAY_Y_OFFSET;
            positions.push([local_x, y, local_z]);

            let position =
                WorldPosition::new(chunk, LocalPosition::new(Vec3::new(local_x, y, local_z)));
            let sample = sample_terrain_field_at(world, catalog, field_id, position);
            let color = if sample.availability.is_available() {
                style.vertex_color_for_value(sample.value, opacity_bp)
            } else {
                let phase = (row + col) % 2 == 0;
                style.unknown_vertex_color(opacity_bp, phase)
            };
            let rgba = color.to_srgba();
            colors.push([rgba.red, rgba.green, rgba.blue, rgba.alpha]);
        }
    }

    let mut indices = Vec::with_capacity((samples_per_edge - 1).pow(2) * 6);
    for row in 0..samples_per_edge - 1 {
        for col in 0..samples_per_edge - 1 {
            let i0 = (row * samples_per_edge + col) as u32;
            let i1 = i0 + 1;
            let i2 = i0 + samples_per_edge as u32;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Heightfield;

    #[test]
    fn overlay_mesh_has_field_resolution_grid() {
        use crate::world::{ChunkLayout, TerrainFieldCatalog, bootstrap_constant_field};
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let samples = vec![0.0f32; 33 * 33];
        let hf = Heightfield::from_samples(33, 8.0, samples).unwrap();
        let mut world = WorldData::new(layout);
        bootstrap_constant_field(
            world.terrain_fields_mut(),
            TerrainFieldId::new("water"),
            ChunkCoord::new(0, 0),
            42_000,
        );
        let catalog = TerrainFieldCatalog::default();
        let style = TerrainFieldOverlayStyle::default();
        let mesh = build_field_overlay_mesh(
            &hf,
            ChunkCoord::new(0, 0),
            &TerrainFieldId::new("water"),
            &world,
            &catalog,
            &style,
            TerrainOverlayState::default().opacity_basis_points,
            1.0,
        );
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(positions.len(), 33 * 33);
    }
}
