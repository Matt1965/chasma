//! Doodad world picking for dev inspector (ADR-098 DT2).

use bevy::prelude::*;

use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::units::input::ray_sphere_hit_distance;
use crate::world::{DoodadCatalog, DoodadId, WorldConfig, WorldData};

use super::components::DoodadRenderEntity;

fn doodad_pick_radius(record: &crate::world::DoodadRecord, catalog: &DoodadCatalog) -> f32 {
    catalog
        .get(&record.definition_id)
        .map(|def| def.placement_radius_meters.max(def.block_radius_meters))
        .unwrap_or(1.0)
        * record
            .placement
            .scale_vec3()
            .x
            .max(record.placement.scale_vec3().z)
}

fn consider_doodad_hit(
    ray: &Ray3d,
    center: Vec3,
    radius: f32,
    doodad_id: DoodadId,
    best: &mut Option<(f32, DoodadId)>,
) {
    let radius = radius.max(0.25);
    let Some(distance) = ray_sphere_hit_distance(ray, center, radius) else {
        return;
    };
    if best.map(|(best_t, _)| distance < best_t).unwrap_or(true) {
        *best = Some((distance, doodad_id));
    }
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

    for (marker, transform) in doodads {
        let Some(record) = world.get_doodad(marker.doodad_id) else {
            continue;
        };
        consider_doodad_hit(
            ray,
            transform.translation(),
            doodad_pick_radius(record, catalog),
            marker.doodad_id,
            &mut best,
        );
    }

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    for doodad_id in world.sorted_doodad_ids() {
        let Some(record) = world.get_doodad(doodad_id) else {
            continue;
        };
        let center =
            world_position_to_render_global(record.placement.position, layout, vertical_scale);
        consider_doodad_hit(
            ray,
            center,
            doodad_pick_radius(record, catalog),
            doodad_id,
            &mut best,
        );
    }

    best.map(|(_, id)| id)
}
