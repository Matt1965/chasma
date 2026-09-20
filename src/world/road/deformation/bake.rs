use std::collections::HashSet;

use bevy::prelude::*;

use crate::world::{
    ChunkCoord, ChunkId, ChunkLayout, Road, RoadNetwork, RoadStyleDefaults, WorldData,
    WorldPosition, sample_road_polyline, try_sample_base_height_at_position,
};
use crate::world::terrain::Heightfield;

use super::store::{
    ROAD_DEFORMATION_BAKE_VERSION, RoadDeformationStore, affected_chunk_coords_for_bounds,
    chunk_ids_from_coords,
};
use super::tile::RoadHeightDeltaTile;

pub const ROAD_BAKE_INFLUENCE_MARGIN_M: f32 = 1.0;

#[derive(Debug, Clone)]
pub struct RoadBakeWarning {
    pub road_id: crate::world::RoadId,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct RoadBakeReport {
    pub baked_chunks: usize,
    pub warnings: Vec<RoadBakeWarning>,
}

struct RoadInfluence {
    road_id: crate::world::RoadId,
    style: RoadStyleDefaults,
    samples: Vec<RoadCenterSample>,
    influence_radius: f32,
}

#[derive(Debug, Clone)]
struct RoadCenterSample {
    position: Vec2,
    distance_m: f32,
    bed_height: f32,
}

pub fn fingerprint_road_network(network: &RoadNetwork) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    network.version.hash(&mut hasher);
    for road in network.roads.values() {
        road.id.as_str().hash(&mut hasher);
        for point in &road.control_points {
            point.x.to_bits().hash(&mut hasher);
            point.z.to_bits().hash(&mut hasher);
        }
        road.style.hash(&mut hasher);
    }
    for junction in network.junctions.values() {
        junction.id.as_str().hash(&mut hasher);
    }
    hasher.finish()
}

pub fn influence_bounds_for_road(road: &Road, style: &RoadStyleDefaults) -> (f32, f32, f32, f32) {
    let half_width = style.width_m * 0.5;
    let radius = half_width + style.shoulder_m + ROAD_BAKE_INFLUENCE_MARGIN_M;
    let mut min_x = f32::INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for point in &road.control_points {
        min_x = min_x.min(point.x - radius);
        min_z = min_z.min(point.z - radius);
        max_x = max_x.max(point.x + radius);
        max_z = max_z.max(point.z + radius);
    }
    (min_x, min_z, max_x, max_z)
}

pub fn affected_chunk_ids_for_network(network: &RoadNetwork, layout: ChunkLayout) -> HashSet<ChunkId> {
    let mut coords = HashSet::new();
    for road in network.roads.values() {
        let defaults = network
            .style_defaults(road.style)
            .cloned()
            .unwrap_or_else(RoadStyleDefaults::dirt_road);
        let style = road.style_overrides.resolve(&defaults);
        let (min_x, min_z, max_x, max_z) = influence_bounds_for_road(road, &style);
        coords.extend(affected_chunk_coords_for_bounds(min_x, min_z, max_x, max_z, layout));
    }
    chunk_ids_from_coords(&coords)
}

/// Ensure `chunk` carries the store tile, lazily baking when the chunk intersects road influence.
pub fn ensure_chunk_road_deformation(
    world: &WorldData,
    store: &mut RoadDeformationStore,
    network: &RoadNetwork,
    layout: ChunkLayout,
    chunk_id: ChunkId,
    chunk: &mut crate::world::ChunkData,
) {
    if !store.tiles.contains_key(&chunk_id) {
        let affected = affected_chunk_ids_for_network(network, layout);
        if affected.contains(&chunk_id) {
            let mut one = HashSet::new();
            one.insert(chunk_id);
            rebake_road_deformation_for_chunks(world, store, network, layout, &one);
        }
    }
    store.apply_tile_to_chunk(chunk_id, chunk);
}

pub fn rebake_road_deformation_for_chunks(
    world: &WorldData,
    store: &mut RoadDeformationStore,
    network: &RoadNetwork,
    layout: ChunkLayout,
    chunk_ids: &HashSet<ChunkId>,
) -> RoadBakeReport {
    let influences = build_road_influences(world, network, layout);
    let mut warnings = Vec::new();
    for influence in &influences {
        if let Some(warning) = validate_road_influence(influence) {
            warnings.push(warning);
        }
    }

    for chunk_id in chunk_ids {
        let tile = if let Some(chunk) = world.get(*chunk_id) {
            bake_chunk_tile(world, layout, *chunk_id, chunk, &influences)
        } else {
            continue;
        };
        store.set_tile(*chunk_id, tile);
    }

    store.bake_version = ROAD_DEFORMATION_BAKE_VERSION;
    store.network_fingerprint = fingerprint_road_network(network);

    RoadBakeReport {
        baked_chunks: chunk_ids.len(),
        warnings,
    }
}

fn build_road_influences(
    world: &WorldData,
    network: &RoadNetwork,
    layout: ChunkLayout,
) -> Vec<RoadInfluence> {
    network
        .roads
        .values()
        .filter(|road| road.control_points.len() >= 2)
        .filter_map(|road| {
            let defaults = network
                .style_defaults(road.style)
                .cloned()
                .unwrap_or_else(RoadStyleDefaults::dirt_road);
            let style = road.style_overrides.resolve(&defaults);
            let spacing = road_sample_spacing(world, layout);
            let polyline = sample_road_polyline(road, spacing);
            if polyline.len() < 2 {
                return None;
            }
            let base_heights = polyline
                .iter()
                .map(|sample| {
                    sample_base_height_world(world, layout, sample.position).unwrap_or(0.0)
                })
                .collect::<Vec<_>>();
            let smoothed = smooth_longitudinal_profile(
                &base_heights,
                &polyline,
                style.longitudinal_smooth_m,
            );
            let bed_heights = base_heights
                .iter()
                .zip(smoothed.iter())
                .map(|(base, smooth)| {
                    let residual = *base - *smooth;
                    *base - style.flatten_strength * residual - style.depression_m
                })
                .collect::<Vec<_>>();
            let samples = polyline
                .iter()
                .zip(bed_heights.iter())
                .map(|(sample, bed)| RoadCenterSample {
                    position: sample.position,
                    distance_m: sample.distance_m,
                    bed_height: *bed,
                })
                .collect();
            let half_width = style.width_m * 0.5;
            let influence_radius = half_width + style.shoulder_m + ROAD_BAKE_INFLUENCE_MARGIN_M;
            Some(RoadInfluence {
                road_id: road.id.clone(),
                style,
                samples,
                influence_radius,
            })
        })
        .collect()
}

fn road_sample_spacing(world: &WorldData, layout: ChunkLayout) -> f32 {
    world
        .iter()
        .next()
        .map(|(_, chunk)| chunk.heightfield.spacing_meters())
        .unwrap_or(layout.chunk_size_meters / 256.0)
        .max(0.5)
}

fn sample_base_height_world(
    world: &WorldData,
    layout: ChunkLayout,
    xz: Vec2,
) -> Option<f32> {
    let position = WorldPosition::from_global(Vec3::new(xz.x, 0.0, xz.y), layout);
    try_sample_base_height_at_position(world, position).ok()
}

fn smooth_longitudinal_profile(
    heights: &[f32],
    samples: &[crate::world::RoadSplineSample],
    window_m: f32,
) -> Vec<f32> {
    heights
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let center = samples[index].distance_m;
            let mut sum = 0.0;
            let mut count = 0.0;
            for (other_index, height) in heights.iter().enumerate() {
                let distance = (samples[other_index].distance_m - center).abs();
                if distance <= window_m {
                    sum += *height;
                    count += 1.0;
                }
            }
            if count > 0.0 { sum / count } else { heights[index] }
        })
        .collect()
}

fn bake_chunk_tile(
    world: &WorldData,
    layout: ChunkLayout,
    chunk_id: ChunkId,
    chunk: &crate::world::ChunkData,
    influences: &[RoadInfluence],
) -> RoadHeightDeltaTile {
    let heightfield = &chunk.heightfield;
    let mut tile = RoadHeightDeltaTile::zero_for_heightfield(heightfield);
    let spe = heightfield.samples_per_edge();
    let spacing = heightfield.spacing_meters();
    let chunk_origin = chunk_world_origin(chunk_id, layout);

    for row in 0..spe {
        for col in 0..spe {
            let local_x = col as f32 * spacing;
            let local_z = row as f32 * spacing;
            let world_x = chunk_origin.x + local_x;
            let world_z = chunk_origin.y + local_z;
            let base = heightfield.height_at_vertex(col, row);
            let delta = compute_delta_at_point(world, layout, Vec2::new(world_x, world_z), base, influences);
            let index = row as usize * spe as usize + col as usize;
            tile.deltas[index] = delta;
        }
    }
    tile
}

fn chunk_world_origin(chunk_id: ChunkId, layout: ChunkLayout) -> Vec2 {
    let coord = chunk_id.coord();
    let size = layout.chunk_size_meters;
    Vec2::new(coord.x as f32 * size, coord.z as f32 * size)
}

fn compute_delta_at_point(
    world: &WorldData,
    layout: ChunkLayout,
    world_xz: Vec2,
    local_base: f32,
    influences: &[RoadInfluence],
) -> f32 {
    let mut weight_sum = 0.0;
    let mut target_sum = 0.0;
    for influence in influences {
        let Some((lateral, distance_m)) = project_onto_road_polyline(world_xz, influence) else {
            continue;
        };
        if lateral > influence.influence_radius {
            continue;
        }
        let weight = cross_falloff_weight(
            lateral,
            influence.style.width_m * 0.5,
            influence.style.shoulder_m,
        );
        if weight <= f32::EPSILON {
            continue;
        }
        let bed = sample_bed_height(distance_m, influence);
        let flatten = influence.style.flatten_strength * weight;
        let target = local_base.lerp(bed, flatten);
        weight_sum += weight;
        target_sum += weight * target;
    }
    if weight_sum <= f32::EPSILON {
        return 0.0;
    }
    let blended_target = target_sum / weight_sum;
    blended_target - local_base
}

fn cross_falloff_weight(lateral: f32, half_width: f32, shoulder: f32) -> f32 {
    if lateral <= half_width {
        1.0
    } else if lateral <= half_width + shoulder {
        let t = (lateral - half_width) / shoulder.max(f32::EPSILON);
        smoothstep(1.0, 0.0, t)
    } else {
        0.0
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 == edge1 {
        return edge0;
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn project_onto_road_polyline(point: Vec2, influence: &RoadInfluence) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for window in influence.samples.windows(2) {
        let start = window[0].position;
        let end = window[1].position;
        let (_projection, lateral, along) = project_point_to_segment(point, start, end);
        let distance_m =
            window[0].distance_m + along * (window[1].distance_m - window[0].distance_m);
        if best
            .as_ref()
            .map(|(_, best_lat)| lateral < *best_lat)
            .unwrap_or(true)
        {
            best = Some((lateral, distance_m));
        }
    }
    best
}

fn project_point_to_segment(point: Vec2, start: Vec2, end: Vec2) -> (Vec2, f32, f32) {
    let segment = end - start;
    let length_sq = segment.length_squared();
    if length_sq <= f32::EPSILON {
        return (start, point.distance(start), 0.0);
    }
    let t = ((point - start).dot(segment) / length_sq).clamp(0.0, 1.0);
    let projection = start + segment * t;
    (projection, point.distance(projection), t)
}

fn sample_bed_height(distance_m: f32, influence: &RoadInfluence) -> f32 {
    if influence.samples.is_empty() {
        return 0.0;
    }
    if distance_m <= influence.samples[0].distance_m {
        return influence.samples[0].bed_height;
    }
    let last = influence.samples.last().expect("samples");
    if distance_m >= last.distance_m {
        return last.bed_height;
    }
    for window in influence.samples.windows(2) {
        let start = &window[0];
        let end = &window[1];
        if distance_m >= start.distance_m && distance_m <= end.distance_m {
            let span = end.distance_m - start.distance_m;
            let t = if span > f32::EPSILON {
                (distance_m - start.distance_m) / span
            } else {
                0.0
            };
            return start.bed_height.lerp(end.bed_height, t);
        }
    }
    last.bed_height
}

fn validate_road_influence(influence: &RoadInfluence) -> Option<RoadBakeWarning> {
    let max_delta = influence
        .samples
        .iter()
        .map(|sample| sample.bed_height.abs())
        .fold(0.0_f32, f32::max);
    if max_delta > 500.0 {
        return Some(RoadBakeWarning {
            road_id: influence.road_id.clone(),
            message: "road bed height displacement unusually large".into(),
        });
    }
    None
}
