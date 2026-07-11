//! Selection debug overlay — enhanced selection highlight rings.

use bevy::prelude::*;

use crate::debug::InspectorOverlayFocus;
use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::terrain::TerrainRenderAssets;
use crate::units::UnitRenderIndex;
use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, WorldData};

use super::helpers::xz_to_render_y;

/// Draw extra gizmo rings at selected unit feet (complements mesh selection indicators).
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

    let mut drawn = 0_u32;

    for unit_id in selection.iter() {
        if drawn >= settings.max_draw_units {
            break;
        }
        let Some(&render_entity) = index.0.get(&unit_id) else {
            continue;
        };
        let Ok(transform) = transforms.get(render_entity) else {
            continue;
        };
        let radius = selection_ring_radius(&world, &catalog, unit_id);
        let center = xz_to_render_y(transform.translation(), 0.05);
        gizmos.circle(
            Isometry3d::new(center, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            radius,
            Color::srgba(0.15, 0.95, 0.25, 0.55),
        );
        drawn += 1;
    }

    if let Some(focus_id) = focus.unit_id {
        if !selection.contains(focus_id) {
            if drawn >= settings.max_draw_units {
                return;
            }
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

fn selection_ring_radius(
    world: &WorldData,
    catalog: &UnitCatalog,
    unit_id: crate::world::UnitId,
) -> f32 {
    let Some(record) = world.get_unit(unit_id) else {
        return 1.0;
    };
    let Some(definition) = catalog.get(&record.definition_id) else {
        return 1.0;
    };
    (definition.collision_radius_meters * 2.0).max(0.9)
}
