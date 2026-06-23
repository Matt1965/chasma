//! Formation debug overlay — per-unit formation offset targets.

use bevy::prelude::*;

use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::SelectedUnits;
use crate::world::{UnitState, WorldConfig, WorldData};

use super::helpers::{render_position, xz_to_render_y};

/// Draw lines from unit positions to their formation move targets.
pub fn draw_formation_debug_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    selection: Res<SelectedUnits>,
    settings: Res<DebugOverlaySettings>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Formation) {
        return;
    }

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let mut drawn = 0_u32;

    for unit_id in selection.iter() {
        if drawn >= settings.max_draw_units {
            break;
        }
        let Some(record) = world.get_unit(unit_id) else {
            continue;
        };
        let UnitState::Moving { target, .. } = record.state else {
            continue;
        };

        let start = xz_to_render_y(
            render_position(record.placement.position, layout, vertical_scale),
            0.18,
        );
        let end = xz_to_render_y(render_position(target, layout, vertical_scale), 0.18);
        gizmos.line(start, end, Color::srgba(0.55, 0.35, 1.0, 0.9));
        gizmos.sphere(end, 0.2, Color::srgba(0.55, 0.35, 1.0, 0.65));
        drawn += 1;
    }
}
