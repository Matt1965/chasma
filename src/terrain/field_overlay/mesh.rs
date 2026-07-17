//! Field overlay mesh generation (ADR-103).
//!
//! Builds a conforming 33×33 vertex grid aligned to authoritative field tiles.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::world::{
    Heightfield, TERRAIN_FIELD_SAMPLES_PER_EDGE, TerrainFieldOverlayStyle, TerrainFieldTile,
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
    field_tile: Option<&TerrainFieldTile>,
    style: &TerrainFieldOverlayStyle,
    opacity_bp: u16,
    vertical_scale: f32,
) -> Mesh {
    let samples_per_edge = TERRAIN_FIELD_SAMPLES_PER_EDGE as usize;
    let spacing = heightfield.spacing_meters();
    let mut positions = Vec::with_capacity(samples_per_edge * samples_per_edge);
    let mut colors = Vec::with_capacity(samples_per_edge * samples_per_edge);
    let hf_samples = heightfield.samples_per_edge() as usize;

    for row in 0..samples_per_edge {
        for col in 0..samples_per_edge {
            let local_x = col as f32 * spacing;
            let local_z = row as f32 * spacing;
            let hf_col = col.min(hf_samples.saturating_sub(1));
            let hf_row = row.min(hf_samples.saturating_sub(1));
            let y = heightfield.samples()[hf_row * hf_samples + hf_col] * vertical_scale
                + OVERLAY_Y_OFFSET;
            positions.push([local_x, y, local_z]);

            let color =
                match field_tile.and_then(|tile| tile.sample_at_vertex(col as u32, row as u32)) {
                    Some(value) => style.vertex_color_for_value(value, opacity_bp),
                    None => {
                        let phase = (row + col) % 2 == 0;
                        style.unknown_vertex_color(opacity_bp, phase)
                    }
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
        let samples = vec![0.0f32; 33 * 33];
        let hf = Heightfield::from_samples(33, 8.0, samples).unwrap();
        let style = TerrainFieldOverlayStyle::default();
        let mesh = build_field_overlay_mesh(
            &hf,
            None,
            &style,
            TerrainOverlayState::default().opacity_basis_points,
            1.0,
        );
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(positions.len(), 33 * 33);
    }
}
