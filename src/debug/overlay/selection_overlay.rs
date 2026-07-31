//! Selection debug overlay — inspector focus highlight (diagnostic only).
//!
//! Normal green unit selection rings are owned by [`crate::player::indicator`].
//! This overlay draws the orange inspector-focus ring when focus diverges from selection.

use bevy::prelude::*;

use crate::debug::InspectorOverlayFocus;
use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::player::selection_ring_radius;
use crate::terrain::TerrainRenderAssets;
use crate::units::UnitRenderIndex;
use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, WorldData};

use super::helpers::xz_to_render_y;

/// Draw inspector-focus diagnostic ring (does not duplicate normal selection presentation).
pub fn draw_selection_debug_overlay(
    mut gizmos: Gizmos,
    selection: Res<SelectedUnits>,
    index: Res<UnitRenderIndex>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    settings: Res<DebugOverlaySettings>,
    focus: Res<InspectorOverlayFocus>,
    _render_assets: Option<Res<TerrainRenderAssets>>,
    transforms: Query<&GlobalTransform>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Selection) {
        return;
    }

    if let Some(focus_id) = focus.unit_id {
        if !selection.contains(focus_id) {
            if let Some(&render_entity) = index.0.get(&focus_id) {
                if let Ok(transform) = transforms.get(render_entity) {
                    let radius = selection_ring_radius(&world, &catalog, focus_id) * 1.15;
                    let center = xz_to_render_y(transform.translation(), 0.08);
                    gizmos.circle(
                        Isometry3d::new(
                            center,
                            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                        ),
                        radius,
                        Color::srgba(0.95, 0.55, 0.15, 0.85),
                    );
                }
            }
        }
    }
}
