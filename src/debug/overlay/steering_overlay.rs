//! Steering debug overlay — sampled separation and cohesion vectors.

use bevy::prelude::*;

use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::SelectedUnits;
use crate::world::{
    SteeringSettings, UnitCatalog, UnitState, WorldConfig, WorldData, cohesion_force,
    gather_steering_neighbors, separation_force,
};

use super::helpers::{render_position, xz_to_render_y};

const STEERING_SETTINGS: SteeringSettings = SteeringSettings::DEFAULT;
const VECTOR_SCALE: f32 = 2.5;

/// Draw separation and cohesion force vectors for selected moving units.
pub fn draw_steering_debug_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    catalog: Res<UnitCatalog>,
    selection: Res<SelectedUnits>,
    settings: Res<DebugOverlaySettings>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Steering) {
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
        let Some(definition) = catalog.get(&record.definition_id) else {
            continue;
        };

        let position = record.placement.position;
        let global = position.to_global(layout);
        let position_xz = Vec2::new(global.x, global.z);
        let target_global = target.to_global(layout);
        let formation_target_xz = Vec2::new(target_global.x, target_global.z);

        let neighbors = gather_steering_neighbors(
            &world,
            &catalog,
            unit_id,
            position,
            STEERING_SETTINGS.neighbor_query_radius,
        );

        let separation = separation_force(
            position_xz,
            definition.collision_radius_meters,
            &neighbors,
            &STEERING_SETTINGS,
        );
        let cohesion = cohesion_force(
            position_xz,
            Some(formation_target_xz),
            &neighbors,
            &STEERING_SETTINGS,
        );

        let origin = xz_to_render_y(render_position(position, layout, vertical_scale), 0.3);
        draw_vector(
            &mut gizmos,
            origin,
            separation,
            Color::srgba(1.0, 0.35, 0.25, 0.9),
        );
        draw_vector(
            &mut gizmos,
            origin,
            cohesion,
            Color::srgba(0.25, 0.75, 1.0, 0.85),
        );
        drawn += 1;
    }
}

fn draw_vector(gizmos: &mut Gizmos, origin: Vec3, force_xz: Vec2, color: Color) {
    if force_xz.length_squared() <= 1e-6 {
        return;
    }
    let end = origin + Vec3::new(force_xz.x, 0.0, force_xz.y) * VECTOR_SCALE;
    gizmos.line(origin, end, color);
    gizmos.sphere(end, 0.08, color);
}
