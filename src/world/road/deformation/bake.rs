use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::terrain::catalog::TerrainWorldCatalog;
use crate::terrain::decode::decode_chunk;
use crate::world::{
    ChunkData, ChunkId, ChunkLayout, Road, RoadNetwork, RoadStyleDefaults, WorldData,
    WorldPosition, sample_road_polyline,
};
use crate::world::road::spline::project_point_onto_road_spline;
use crate::world::terrain::Heightfield;

use super::store::{
    ROAD_DEFORMATION_BAKE_VERSION, RoadDeformationStore, affected_chunk_coords_for_bounds,
    chunk_ids_from_coords,
};
use super::tile::RoadHeightDeltaTile;

pub const ROAD_BAKE_INFLUENCE_MARGIN_M: f32 = 1.0;
/// Warn when a single vertex delta exceeds this (unexpected after correct base sampling).
pub const ROAD_DELTA_WARN_ABS_M: f32 = 8.0;

#[derive(Debug, Clone)]
pub struct RoadBakeWarning {
    pub road_id: crate::world::RoadId,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct RoadBakeReport {
    pub roads: usize,
    pub dirty_chunks: usize,
    pub baked_chunks: usize,
    pub skipped_unloaded_chunks: usize,
    pub cleared_stale_chunks: usize,
    pub nonzero_tiles: usize,
    pub delta_min: f32,
    pub delta_max: f32,
    pub warnings: Vec<RoadBakeWarning>,
}

impl RoadBakeReport {
    pub fn summary_line(&self) -> String {
        format!(
            "Road deformation bake: roads={} dirty={} baked={} skipped={} cleared={} nonzero_tiles={} delta=[{:.3}, {:.3}]",
            self.roads,
            self.dirty_chunks,
            self.baked_chunks,
            self.skipped_unloaded_chunks,
            self.cleared_stale_chunks,
            self.nonzero_tiles,
            self.delta_min,
            self.delta_max,
        )
    }
}

struct RoadInfluence {
    road_id: crate::world::RoadId,
    style: RoadStyleDefaults,
    polyline_spacing_m: f32,
    samples: Vec<RoadCenterSample>,
    influence_radius: f32,
}

#[derive(Debug, Clone)]
struct RoadCenterSample {
    position: Vec2,
    distance_m: f32,
    bed_height: f32,
}

/// Samples base terrain from resident world chunks plus optional bake-time chunk payloads.
struct BakeTerrainSampler {
    layout: ChunkLayout,
    resident: HashMap<ChunkId, Heightfield>,
}

impl BakeTerrainSampler {
    fn for_rebake(
        world: &WorldData,
        layout: ChunkLayout,
        extra_chunks: &HashMap<ChunkId, ChunkData>,
    ) -> Self {
        let mut resident = HashMap::new();
        for (chunk_id, chunk) in world.iter() {
            resident.insert(chunk_id, chunk.heightfield.clone());
        }
        for (chunk_id, chunk) in extra_chunks {
            resident
                .entry(*chunk_id)
                .or_insert_with(|| chunk.heightfield.clone());
        }
        Self { layout, resident }
    }

    fn sample_base(&self, xz: Vec2) -> Option<f32> {
        let position = WorldPosition::from_global(Vec3::new(xz.x, 0.0, xz.y), self.layout);
        let chunk_id = ChunkId::new(position.chunk);
        let heightfield = self.resident.get(&chunk_id)?;
        heightfield
            .try_sample(position.local.0.x, position.local.0.z)
            .ok()
    }
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

pub fn influence_bounds_for_polyline(
    polyline: &[crate::world::RoadSplineSample],
    radius: f32,
) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for sample in polyline {
        min_x = min_x.min(sample.position.x - radius);
        min_z = min_z.min(sample.position.y - radius);
        max_x = max_x.max(sample.position.x + radius);
        max_z = max_z.max(sample.position.y + radius);
    }
    if polyline.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (min_x, min_z, max_x, max_z)
}

pub fn influence_bounds_for_road(
    road: &Road,
    style: &RoadStyleDefaults,
    layout: ChunkLayout,
) -> (f32, f32, f32, f32) {
    let half_width = style.width_m * 0.5;
    let radius = half_width + style.shoulder_m + ROAD_BAKE_INFLUENCE_MARGIN_M;
    let spacing = default_road_sample_spacing(layout);
    let polyline = sample_road_polyline(road, spacing);
    if polyline.is_empty() {
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
        return (min_x, min_z, max_x, max_z);
    }
    influence_bounds_for_polyline(&polyline, radius)
}

pub fn affected_chunk_ids_for_network(network: &RoadNetwork, layout: ChunkLayout) -> HashSet<ChunkId> {
    let mut coords = HashSet::new();
    for road in network.roads.values() {
        let defaults = network
            .style_defaults(road.style)
            .cloned()
            .unwrap_or_else(RoadStyleDefaults::dirt_road);
        let style = road.style_overrides.resolve(&defaults);
        let (min_x, min_z, max_x, max_z) = influence_bounds_for_road(road, &style, layout);
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
    catalog: Option<&TerrainWorldCatalog>,
) {
    if !store.tiles.contains_key(&chunk_id) {
        let affected = affected_chunk_ids_for_network(network, layout);
        if affected.contains(&chunk_id) {
            let mut one = HashSet::new();
            one.insert(chunk_id);
            rebake_road_deformation_for_chunks(world, store, network, layout, &one, catalog);
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
    catalog: Option<&TerrainWorldCatalog>,
) -> RoadBakeReport {
    let mut extra_chunks = HashMap::new();
    for chunk_id in chunk_ids {
        if world.get(*chunk_id).is_some() {
            continue;
        }
        if let Some(chunk) = load_chunk_data_from_catalog(catalog, *chunk_id) {
            extra_chunks.insert(*chunk_id, chunk);
        }
    }

    let sampler = BakeTerrainSampler::for_rebake(world, layout, &extra_chunks);
    let influences = build_road_influences(&sampler, network, layout);
    let roads = &network.roads;
    let mut warnings = Vec::new();
    for influence in &influences {
        if let Some(warning) = validate_road_influence(influence) {
            warnings.push(warning);
        }
    }

    let mut report = RoadBakeReport {
        roads: network.roads.len(),
        dirty_chunks: chunk_ids.len(),
        ..Default::default()
    };
    let mut delta_min = f32::INFINITY;
    let mut delta_max = f32::NEG_INFINITY;

    for chunk_id in chunk_ids {
        let chunk_data = world
            .get(*chunk_id)
            .cloned()
            .or_else(|| extra_chunks.get(chunk_id).cloned());
        match chunk_data {
            Some(chunk) => {
                let tile = bake_chunk_tile(
                    layout,
                    *chunk_id,
                    &chunk,
                    roads,
                    &influences,
                    &mut warnings,
                );
                accumulate_tile_delta_stats(&tile, &mut delta_min, &mut delta_max);
                store.set_tile(*chunk_id, tile);
                report.baked_chunks += 1;
            }
            None => {
                let mut one = HashSet::new();
                one.insert(*chunk_id);
                if store.tiles.contains_key(chunk_id) {
                    store.remove_chunks(&one);
                    report.cleared_stale_chunks += 1;
                }
                report.skipped_unloaded_chunks += 1;
            }
        }
    }

    report.nonzero_tiles = store
        .tiles
        .values()
        .filter(|tile| !tile.is_effectively_zero())
        .count();
    if delta_min.is_finite() {
        report.delta_min = delta_min;
        report.delta_max = delta_max;
    }

    store.bake_version = ROAD_DEFORMATION_BAKE_VERSION;
    store.network_fingerprint = fingerprint_road_network(network);
    report.warnings = warnings;
    report
}

fn load_chunk_data_from_catalog(
    catalog: Option<&TerrainWorldCatalog>,
    chunk_id: ChunkId,
) -> Option<crate::world::ChunkData> {
    let catalog = catalog?;
    let coord = chunk_id.coord();
    let path = catalog.chunk_path(coord)?;
    let text = std::fs::read_to_string(&path).map_err(|error| {
        bevy::log::warn!(
            "road deformation bake: failed to read chunk ({}, {}): {error}",
            coord.x,
            coord.z
        );
    }).ok()?;
    let (id, data) = decode_chunk(&text).map_err(|error| {
        bevy::log::warn!(
            "road deformation bake: failed to decode chunk ({}, {}): {error}",
            coord.x,
            coord.z
        );
    }).ok()?;
    if id != chunk_id {
        bevy::log::warn!(
            "road deformation bake: chunk id mismatch for ({}, {})",
            coord.x,
            coord.z
        );
        return None;
    }
    Some(data)
}

fn build_road_influences(
    sampler: &BakeTerrainSampler,
    network: &RoadNetwork,
    layout: ChunkLayout,
) -> Vec<RoadInfluence> {
    let spacing = road_sample_spacing(sampler, layout);
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
            let polyline = sample_road_polyline(road, spacing);
            if polyline.len() < 2 {
                return None;
            }
            let raw_base = polyline
                .iter()
                .map(|sample| sampler.sample_base(sample.position))
                .collect::<Vec<_>>();
            let base_heights = fill_missing_centerline_base_heights(&raw_base)?;
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
                polyline_spacing_m: spacing,
                samples,
                influence_radius,
            })
        })
        .collect()
}

fn road_sample_spacing(sampler: &BakeTerrainSampler, layout: ChunkLayout) -> f32 {
    sampler
        .resident
        .values()
        .next()
        .map(|heightfield| heightfield.spacing_meters())
        .unwrap_or(layout.chunk_size_meters / 256.0)
        .max(0.5)
}

fn default_road_sample_spacing(layout: ChunkLayout) -> f32 {
    (layout.chunk_size_meters / 256.0).max(0.5)
}

/// Edge-safe fill: never treat missing samples as zero. Clamp endpoints and linearly bridge gaps.
fn fill_missing_centerline_base_heights(raw: &[Option<f32>]) -> Option<Vec<f32>> {
    let n = raw.len();
    if n == 0 {
        return None;
    }
    let first_known = raw.iter().position(|v| v.is_some());
    let last_known = raw.iter().rposition(|v| v.is_some());
    let (first_known, last_known) = match (first_known, last_known) {
        (Some(f), Some(l)) => (f, l),
        _ => return None,
    };

    let mut out = vec![0.0; n];
    let first_val = raw[first_known].unwrap();
    for i in 0..first_known {
        out[i] = first_val;
    }
    let last_val = raw[last_known].unwrap();
    for i in last_known + 1..n {
        out[i] = last_val;
    }

    let mut i = first_known;
    out[i] = raw[i].unwrap();
    while i < last_known {
        let start = i;
        let start_val = out[start];
        let mut j = start + 1;
        while j <= last_known && raw[j].is_none() {
            j += 1;
        }
        if j > last_known {
            break;
        }
        let end_val = raw[j].unwrap();
        let span = j - start;
        for k in 1..span {
            let t = k as f32 / span as f32;
            out[start + k] = start_val.lerp(end_val, t);
        }
        out[j] = end_val;
        i = j;
    }
    Some(out)
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
            if count > 0.0 {
                sum / count
            } else {
                heights[index]
            }
        })
        .collect()
}

fn bake_chunk_tile(
    layout: ChunkLayout,
    chunk_id: ChunkId,
    chunk: &crate::world::ChunkData,
    roads: &std::collections::BTreeMap<crate::world::RoadId, Road>,
    influences: &[RoadInfluence],
    warnings: &mut Vec<RoadBakeWarning>,
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
            let delta = compute_delta_at_point(
                Vec2::new(world_x, world_z),
                base,
                roads,
                influences,
            );
            if !delta.is_finite() {
                warnings.push(RoadBakeWarning {
                    road_id: crate::world::RoadId::new("chunk"),
                    message: format!(
                        "non-finite road delta at chunk ({}, {}) col={} row={}",
                        chunk_id.coord().x,
                        chunk_id.coord().z,
                        col,
                        row
                    ),
                });
            } else if delta.abs() > ROAD_DELTA_WARN_ABS_M {
                warnings.push(RoadBakeWarning {
                    road_id: crate::world::RoadId::new("chunk"),
                    message: format!(
                        "large road delta {:.2}m at chunk ({}, {}) col={} row={}",
                        delta,
                        chunk_id.coord().x,
                        chunk_id.coord().z,
                        col,
                        row
                    ),
                });
            }
            let index = row as usize * spe as usize + col as usize;
            tile.deltas[index] = if delta.is_finite() { delta } else { 0.0 };
        }
    }
    tile
}

fn accumulate_tile_delta_stats(tile: &RoadHeightDeltaTile, min: &mut f32, max: &mut f32) {
    for delta in &tile.deltas {
        if !delta.is_finite() {
            continue;
        }
        *min = min.min(*delta);
        *max = max.max(*delta);
    }
}

fn chunk_world_origin(chunk_id: ChunkId, layout: ChunkLayout) -> Vec2 {
    let coord = chunk_id.coord();
    let size = layout.chunk_size_meters;
    Vec2::new(coord.x as f32 * size, coord.z as f32 * size)
}

fn compute_delta_at_point(
    world_xz: Vec2,
    local_base: f32,
    roads: &std::collections::BTreeMap<crate::world::RoadId, Road>,
    influences: &[RoadInfluence],
) -> f32 {
    let mut weight_sum = 0.0;
    let mut target_sum = 0.0;
    for influence in influences {
        let Some(road) = roads.get(&influence.road_id) else {
            continue;
        };
        let Some(projection) =
            project_point_onto_road_spline(road, world_xz, influence.polyline_spacing_m)
        else {
            continue;
        };
        let lateral = projection.distance_to_point;
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
        let bed = sample_bed_height(projection.distance_m, influence);
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
    for sample in &influence.samples {
        if !sample.bed_height.is_finite() {
            return Some(RoadBakeWarning {
                road_id: influence.road_id.clone(),
                message: "road bed height is non-finite".into(),
            });
        }
    }
    let min_bed = influence
        .samples
        .iter()
        .map(|s| s.bed_height)
        .fold(f32::INFINITY, f32::min);
    let max_bed = influence
        .samples
        .iter()
        .map(|s| s.bed_height)
        .fold(f32::NEG_INFINITY, f32::max);
    if max_bed - min_bed > 500.0 {
        return Some(RoadBakeWarning {
            road_id: influence.road_id.clone(),
            message: "road bed height range unusually large".into(),
        });
    }
    None
}

#[cfg(test)]
pub(crate) fn fill_missing_centerline_base_heights_for_test(
    raw: &[Option<f32>],
) -> Option<Vec<f32>> {
    fill_missing_centerline_base_heights(raw)
}

#[cfg(test)]
mod bake_tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, Heightfield, Road, RoadControlPoint, RoadId, RoadNetwork,
        RoadStyleId, RoadStyleOverrides, WorldData,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    #[test]
    fn rebake_sync_and_effective_height_for_simple_road() {
        use crate::world::terrain::try_sample_height_at_position;
        use crate::world::{LocalPosition, WorldPosition};
        use super::super::sync_store_tiles_to_chunks;

        let heightfield = Heightfield::from_samples(5, 64.0, vec![10.0; 25]).unwrap();
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let mut network = RoadNetwork::empty();
        network.roads.insert(
            RoadId::new("r1"),
            Road {
                id: RoadId::new("r1"),
                display_name: String::new(),
                style: RoadStyleId::DirtRoad,
                style_overrides: RoadStyleOverrides::default(),
                control_points: vec![
                    RoadControlPoint::new(64.0, 128.0),
                    RoadControlPoint::new(192.0, 128.0),
                ],
                start_attachment: None,
                end_attachment: None,
                tee_attachments: Vec::new(),
            },
        );
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        let mut store = RoadDeformationStore::default();
        let mut dirty = HashSet::new();
        dirty.insert(chunk_id);
        rebake_road_deformation_for_chunks(
            &world,
            &mut store,
            &network,
            layout(),
            &dirty,
            None,
        );
        let store_tile = store.tiles.get(&chunk_id).expect("store tile");
        let store_nonzero = store_tile.deltas.iter().filter(|d| d.abs() > 1e-5).count();
        assert!(store_nonzero > 0, "store nonzero {}", store_nonzero);
        let store_delta = store_tile
            .sample_delta(128.0, 128.0)
            .expect("store sample");
        assert!(store_delta.abs() > 1e-5, "store delta at center {}", store_delta);

        sync_store_tiles_to_chunks(&store, &mut world, &dirty);
        let chunk_tile = world
            .get(chunk_id)
            .and_then(|chunk| chunk.road_height_delta.clone())
            .expect("chunk tile after sync");
        let chunk_delta = chunk_tile
            .sample_delta(128.0, 128.0)
            .expect("chunk sample");
        assert!(
            chunk_delta.abs() > 1e-5,
            "chunk delta at center {}",
            chunk_delta
        );

        let on_road = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
        );
        let base = 10.0;
        let effective = try_sample_height_at_position(&world, on_road).unwrap();
        assert!(
            effective < base,
            "effective {} should be below base {} (chunk_delta={})",
            effective,
            base,
            chunk_delta
        );
    }

    #[test]
    fn rebake_path_matches_direct_bake_for_simple_road() {
        let heightfield = Heightfield::from_samples(5, 64.0, vec![10.0; 25]).unwrap();
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let mut network = RoadNetwork::empty();
        network.roads.insert(
            RoadId::new("r1"),
            Road {
                id: RoadId::new("r1"),
                display_name: String::new(),
                style: RoadStyleId::DirtRoad,
                style_overrides: RoadStyleOverrides::default(),
                control_points: vec![
                    RoadControlPoint::new(64.0, 128.0),
                    RoadControlPoint::new(192.0, 128.0),
                ],
                start_attachment: None,
                end_attachment: None,
                tee_attachments: Vec::new(),
            },
        );
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        let mut store = RoadDeformationStore::default();
        let mut dirty = HashSet::new();
        dirty.insert(chunk_id);
        let report = rebake_road_deformation_for_chunks(
            &world,
            &mut store,
            &network,
            layout(),
            &dirty,
            None,
        );
        let tile = store.tiles.get(&chunk_id).expect("store tile after rebake");
        let nonzero = tile.deltas.iter().filter(|d| d.abs() > 1e-5).count();
        assert!(report.baked_chunks == 1, "baked {}", report.baked_chunks);
        assert!(nonzero > 0, "rebake nonzero {}", nonzero);
    }

    #[test]
    fn build_influences_and_delta_for_simple_road() {
        let heightfield = Heightfield::from_samples(5, 64.0, vec![10.0; 25]).unwrap();
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let mut network = RoadNetwork::empty();
        network.roads.insert(
            RoadId::new("r1"),
            Road {
                id: RoadId::new("r1"),
                display_name: String::new(),
                style: RoadStyleId::DirtRoad,
                style_overrides: RoadStyleOverrides::default(),
                control_points: vec![
                    RoadControlPoint::new(64.0, 128.0),
                    RoadControlPoint::new(192.0, 128.0),
                ],
                start_attachment: None,
                end_attachment: None,
                tee_attachments: Vec::new(),
            },
        );
        let sampler = BakeTerrainSampler::for_rebake(&world, layout(), &HashMap::new());
        let influences = build_road_influences(&sampler, &network, layout());
        assert!(!influences.is_empty(), "expected road influence");
        let chunk = world.get(ChunkId::new(ChunkCoord::new(0, 0))).unwrap();
        let tile = bake_chunk_tile(
            layout(),
            ChunkId::new(ChunkCoord::new(0, 0)),
            chunk,
            &network.roads,
            &influences,
            &mut Vec::new(),
        );
        let nonzero = tile.deltas.iter().filter(|d| d.abs() > 1e-5).count();
        assert!(nonzero > 0, "expected nonzero road deltas, got {}", nonzero);
        let center = tile.delta_at_vertex(2, 2);
        assert!(
            center.abs() > 1e-5,
            "expected center vertex delta, got {} grid={:?}",
            center,
            tile.deltas
        );
    }
}
