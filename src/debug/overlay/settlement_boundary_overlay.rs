//! TEMPORARY: always-visible settlement boundary visualization during milestone development.
//! Replace with conditional Dev / Build Mode visibility when player placement lands.

use bevy::prelude::*;

use crate::player::selection_ring_mesh::{
    SELECTION_RING_SEGMENTS, draw_terrain_ring_gizmos, sample_terrain_ring_render_points,
};
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{WorldConfig, WorldData};

#[cfg(feature = "dev")]
pub fn draw_settlement_boundary_overlay(
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut gizmos: Gizmos,
) {
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    for settlement_id in world.settlement_store().sorted_settlement_ids() {
        let Some(settlement) = world.settlement_store().get_settlement(settlement_id) else {
            continue;
        };
        let center = world_position_to_render_global(settlement.center, layout, vertical_scale);
        let radius = settlement.boundary_radius_meters.max(0.1);
        let ring = sample_terrain_ring_render_points(
            center,
            radius,
            &world,
            layout,
            vertical_scale,
            SELECTION_RING_SEGMENTS,
        );
        draw_terrain_ring_gizmos(&mut gizmos, &ring, Color::srgba(0.95, 0.82, 0.2, 0.85));
        gizmos.sphere(
            center + Vec3::Y * 0.35,
            0.4,
            Color::srgba(0.95, 0.82, 0.2, 0.35),
        );
    }
}
