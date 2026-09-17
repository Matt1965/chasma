//! DEV-only placement debug gizmos (plane normal + foundation perimeter).

use bevy::prelude::*;

use crate::world::{
    FootprintCatalog, TerrainPlacementMode, WorldData, effective_building_footprint_for_placement,
    sample_terrain_under_footprint,
};

use super::placement_trace::BuildModePlacementTrace;
use super::state::BuildModeState;

pub fn draw_build_mode_placement_gizmos(
    mut gizmos: Gizmos,
    build_mode: Res<BuildModeState>,
    trace: Res<BuildModePlacementTrace>,
    world: Res<WorldData>,
    building_catalog: Res<crate::world::BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    config: Res<crate::world::WorldConfig>,
    render_assets: Option<Res<crate::terrain::TerrainRenderAssets>>,
) {
    if !trace.enabled || !build_mode.is_ghost_placing() {
        return;
    }
    let Some(definition_id) = build_mode.ghost_definition_id() else {
        return;
    };
    let Some(definition) = building_catalog.get(definition_id) else {
        return;
    };
    let Some(plan) = build_mode.last_plan.as_ref() else {
        return;
    };
    if !plan.is_valid() {
        return;
    }

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let anchor_global = plan.grounded_anchor.to_global(layout);
    let anchor_render = Vec3::new(
        anchor_global.x,
        crate::terrain::render_height(anchor_global.y, vertical_scale),
        anchor_global.z,
    );
    let yaw = plan.quantized_rotation.radians();

    let Ok(shape) =
        effective_building_footprint_for_placement(definition, &footprint_catalog, 1.0)
    else {
        return;
    };
    let anchor_xz = Vec2::new(anchor_global.x, anchor_global.z);
    let Ok(report) = sample_terrain_under_footprint(
        &world,
        layout,
        shape.as_ref(),
        anchor_xz,
        yaw,
    ) else {
        return;
    };

    match definition.terrain_placement_mode {
        TerrainPlacementMode::ConformToTerrain => {
            let up = plan.rotation * Vec3::Y;
            let tip = anchor_render + up.normalize_or_zero() * 3.0;
            gizmos.line(anchor_render, tip, Color::srgb(0.2, 1.0, 0.3));
        }
        TerrainPlacementMode::LevelFoundation => {
            let color = Color::srgb(1.0, 0.85, 0.2);
            let perimeter = &report.perimeter;
            if perimeter.len() < 2 {
                return;
            }
            for i in 0..perimeter.len() {
                let a = perimeter[i];
                let b = perimeter[(i + 1) % perimeter.len()];
                let ay = crate::terrain::render_height(a.height, vertical_scale);
                let by = crate::terrain::render_height(b.height, vertical_scale);
                gizmos.line(
                    Vec3::new(a.global_xz.x, ay + 0.1, a.global_xz.y),
                    Vec3::new(b.global_xz.x, by + 0.1, b.global_xz.y),
                    color,
                );
            }
        }
    }
}
