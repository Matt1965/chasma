//! Doodad world picking for dev inspector (ADR-098 DT2).

use bevy::prelude::*;

use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::units::input::ray_sphere_hit_distance;
use crate::world::{
    DoodadCatalog, DoodadId, WorldConfig, WorldData, doodad_interaction_radius_meters,
};

use super::components::DoodadRenderEntity;

/// Floor so small props remain clickable.
const MIN_DOODAD_PICK_RADIUS_METERS: f32 = 2.0;

/// Raise the pick sphere so canopy / upper mesh clicks hit tall props (trees).
const DOODAD_PICK_CENTER_Y_FACTOR: f32 = 0.5;

fn doodad_pick_radius(
    record: &crate::world::DoodadRecord,
    catalog: &DoodadCatalog,
) -> f32 {
    catalog
        .get(&record.definition_id)
        .map(|definition| doodad_interaction_radius_meters(record, definition))
        .unwrap_or(2.0)
        .max(MIN_DOODAD_PICK_RADIUS_METERS)
}

fn doodad_pick_center(base: Vec3, radius: f32, visual_height: f32) -> Vec3 {
    let lift = (radius * DOODAD_PICK_CENTER_Y_FACTOR)
        .max(visual_height * 0.35)
        .clamp(1.5, 12.0);
    base + Vec3::Y * lift
}

fn doodad_visual_height(
    record: &crate::world::DoodadRecord,
    catalog: &DoodadCatalog,
) -> f32 {
    catalog
        .get(&record.definition_id)
        .map(|definition| {
            definition
                .asset_sizing
                .approximate_final_dimensions_meters()
                .map(|dims| dims.height_meters)
                .unwrap_or_else(|| {
                    crate::world::doodad_final_render_scale(
                        definition,
                        record.placement.scale_vec3(),
                    )
                    .y
                })
        })
        .unwrap_or(6.0)
}

fn consider_doodad_hit(
    ray: &Ray3d,
    center: Vec3,
    radius: f32,
    doodad_id: DoodadId,
    best: &mut Option<(f32, DoodadId)>,
) {
    let radius = radius.max(MIN_DOODAD_PICK_RADIUS_METERS);
    let Some(distance) = ray_sphere_hit_distance(ray, center, radius) else {
        return;
    };
    if best.map(|(best_t, _)| distance < best_t).unwrap_or(true) {
        *best = Some((distance, doodad_id));
    }
}

fn consider_doodad_at_position(
    ray: &Ray3d,
    base: Vec3,
    record: &crate::world::DoodadRecord,
    catalog: &DoodadCatalog,
    doodad_id: DoodadId,
    best: &mut Option<(f32, DoodadId)>,
) {
    let radius = doodad_pick_radius(record, catalog);
    let height = doodad_visual_height(record, catalog);
    consider_doodad_hit(
        ray,
        doodad_pick_center(base, radius, height),
        radius.max(height * 0.2),
        doodad_id,
        best,
    );
}

/// Pick the nearest doodad along a screen ray using render entities, with world-data fallback.
pub fn pick_doodad_along_ray(
    ray: &Ray3d,
    world: &WorldData,
    catalog: &DoodadCatalog,
    config: &WorldConfig,
    render_assets: &Option<Res<TerrainRenderAssets>>,
    doodads: &Query<(&DoodadRenderEntity, &GlobalTransform)>,
) -> Option<DoodadId> {
    let mut best: Option<(f32, DoodadId)> = None;
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    for doodad_id in world.sorted_doodad_ids() {
        let Some(record) = world.get_doodad(doodad_id) else {
            continue;
        };
        let world_base =
            world_position_to_render_global(record.placement.position, layout, vertical_scale);
        consider_doodad_at_position(ray, world_base, record, catalog, doodad_id, &mut best);
    }

    for (marker, transform) in doodads {
        let Some(record) = world.get_doodad(marker.doodad_id) else {
            continue;
        };
        let render_base = transform.translation();
        let world_base =
            world_position_to_render_global(record.placement.position, layout, vertical_scale);
        if render_base.distance(world_base) > 0.25 {
            consider_doodad_at_position(
                ray,
                render_base,
                record,
                catalog,
                marker.doodad_id,
                &mut best,
            );
        }
    }

    best.map(|(_, id)| id)
}
