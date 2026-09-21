use bevy::prelude::*;

use crate::dev::dev_mode::DevModeState;
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{OriginCatalog, WorldConfig, WorldData, WorldPosition, ground_world_position};

use super::state::DevOriginEditorState;

const ANCHOR_RADIUS: f32 = 0.75;
const FACING_ARROW_LENGTH: f32 = 3.0;

pub fn draw_origin_editor_gizmos(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    editor: Res<DevOriginEditorState>,
    origins: Res<OriginCatalog>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut gizmos: Gizmos,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::OriginEditor) {
        return;
    }
    let definitions = origins.definitions();
    if definitions.is_empty() {
        return;
    }
    let index = editor
        .selected_index
        .min(definitions.len().saturating_sub(1));
    let origin = &definitions[index];
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let candidate = WorldPosition::from_global(
        Vec3::new(origin.start_x, 0.0, origin.start_z),
        layout,
    );
    let world_pos = ground_world_position(&world, candidate).unwrap_or(candidate);
    let anchor = world_position_to_render_global(world_pos, layout, vertical_scale);
    gizmos.sphere(
        Isometry3d::from_translation(anchor + Vec3::Y * 0.2),
        ANCHOR_RADIUS,
        Color::srgba(0.95, 0.75, 0.2, 0.9),
    );
    let yaw = origin.yaw_deg.to_radians();
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let tip = anchor + forward * FACING_ARROW_LENGTH + Vec3::Y * 0.2;
    gizmos.line(anchor + Vec3::Y * 0.2, tip, Color::srgba(0.2, 0.9, 0.95, 0.95));
    gizmos.sphere(
        Isometry3d::from_translation(tip),
        0.35,
        Color::srgba(0.2, 0.9, 0.95, 0.95),
    );
}
