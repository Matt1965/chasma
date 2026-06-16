//! Chunk residency streaming policy (ADR-012).
//!
//! Pure functions for desired-set computation and residency diffs.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::view::PrimaryViewFocus;
use crate::world::{ChunkCoord, ChunkId, ChunkLayout, WorldData, WorldPosition};

use super::catalog::TerrainWorldCatalog;

/// Tunable streaming parameters (ADR-012, Phase 2B.5 step 4).
///
/// Controls **which chunks exist** in [`WorldData`] and as render entities.
/// [`super::lod::TerrainLodSettings`] is separate: it only picks mesh resolution
/// among chunks already within the keep ring below.
#[derive(Debug, Clone, Resource, Reflect)]#[reflect(Resource)]
pub struct TerrainStreamingSettings {
    /// Chebyshev radius around the view focus used to **request** chunk loads.
    ///
    /// This is the inner ring: chunks within this distance begin loading when
    /// absent. Defines how far terrain is **requested** — not mesh detail.
    /// Must be `<= unload_radius_chunks` (the outer keep ring).
    pub load_radius_chunks: i32,
    /// Chebyshev radius within which resident chunks are **kept** loaded.
    ///
    /// This is the outer retention ring and bounds **visible terrain distance**.
    /// Must be `>= load_radius_chunks`. Chunks between the two radii stay resident
    /// once loaded (hysteresis band). LOD rings do not extend beyond this.
    pub unload_radius_chunks: i32,
    pub max_loads_per_frame: usize,
    pub max_unloads_per_frame: usize,
    /// Maximum decoded chunks applied to [`WorldData`] per frame.
    pub max_apply_per_frame: usize,
    /// Soft cap on IO→decode transitions per poll tick (`0` = unlimited).
    pub max_decode_per_frame: usize,
}

impl Default for TerrainStreamingSettings {
    fn default() -> Self {
        Self {
            load_radius_chunks: 1,
            unload_radius_chunks: 2,
            max_loads_per_frame: 2,
            max_unloads_per_frame: 4,
            max_apply_per_frame: 2,
            max_decode_per_frame: 4,
        }
    }
}

/// Chebyshev distance between chunk coordinates on the horizontal grid.
pub fn chunk_chebyshev_distance(a: ChunkCoord, b: ChunkCoord) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}

/// Chunk coordinate containing the view focus position.
pub fn focus_chunk(focus: Vec3, layout: ChunkLayout) -> ChunkCoord {
    WorldPosition::from_global(focus, layout).chunk
}

/// Snap horizontal focus to a coarse grid so tiny camera jitter does not flip chunk coords.
fn quantize_focus_horizontal(position: Vec3, layout: ChunkLayout) -> Vec3 {
    let step = (layout.chunk_size_units() / 128.0).max(0.25);
    let snap = |v: f32| (v / step).round() * step;
    Vec3::new(snap(position.x), position.y, snap(position.z))
}

/// Stable focus chunk for streaming (quantized horizontal position).
pub fn stable_focus_chunk(position: Vec3, layout: ChunkLayout) -> ChunkCoord {
    focus_chunk(quantize_focus_horizontal(position, layout), layout)
}

/// Authored chunks within `radius_chunks` of `focus` (O(r²), catalog-local).
///
/// Does not scan the full manifest — only the `(2r+1)²` coordinate neighborhood.
pub fn chunks_in_radius(
    focus: ChunkCoord,
    radius_chunks: i32,
    catalog: &TerrainWorldCatalog,
) -> HashSet<ChunkCoord> {
    let mut out = HashSet::new();
    for dz in -radius_chunks..=radius_chunks {
        for dx in -radius_chunks..=radius_chunks {
            let coord = ChunkCoord::new(focus.x + dx, focus.z + dz);
            if catalog.contains(coord) {
                out.insert(coord);
            }
        }
    }
    out
}

/// Compute load and unload worklists for one streaming tick.
///
/// Hysteresis (load ring inside keep ring):
/// - `desired_load_set` = authored chunks within `load_radius_chunks` (inner)
/// - `keep_resident_set` = authored chunks within `unload_radius_chunks` (outer)
/// - `to_load` = `desired_load_set` − resident set
/// - `to_unload` = resident set − `keep_resident_set` − `unload_exempt`
///
/// Requires `unload_radius_chunks >= load_radius_chunks`. Chunks in the band
/// between the two radii remain resident once loaded and do not ping-pong.
pub fn diff_streaming_residency(
    focus: &PrimaryViewFocus,
    layout: ChunkLayout,
    settings: &TerrainStreamingSettings,
    catalog: &TerrainWorldCatalog,
    world: &WorldData,
    unload_exempt: &HashSet<ChunkId>,
) -> (Vec<ChunkCoord>, Vec<ChunkId>) {
    debug_assert!(
        settings.unload_radius_chunks >= settings.load_radius_chunks,
        "unload_radius_chunks (keep ring) must be >= load_radius_chunks (load ring)"
    );

    let focus = stable_focus_chunk(focus.position, layout);
    let desired_load = chunks_in_radius(focus, settings.load_radius_chunks, catalog);
    let keep_resident = chunks_in_radius(focus, settings.unload_radius_chunks, catalog);

    let mut to_load: Vec<_> = desired_load
        .iter()
        .filter(|coord| !world.is_chunk_loaded(ChunkId::new(**coord)))
        .copied()
        .collect();
    let mut to_unload: Vec<_> = world
        .iter()
        .map(|(id, _)| id)
        .filter(|id| {
            !keep_resident.contains(&id.coord()) && !unload_exempt.contains(id)
        })
        .collect();

    to_load.sort_by_key(|c| (c.z, c.x));
    to_unload.sort_by_key(|id| (id.coord().z, id.coord().x));

    to_load.truncate(settings.max_loads_per_frame);
    to_unload.truncate(settings.max_unloads_per_frame);

    debug_assert!(
        !to_load.iter().any(|coord| {
            to_unload
                .iter()
                .any(|id| id.coord() == *coord)
        }),
        "a chunk must not be scheduled to load and unload in the same frame"
    );

    (to_load, to_unload)
}

/// Returns true when a chunk coord is outside the keep or desired load rings.
pub(crate) fn chunk_outside_residency_sets(
    coord: ChunkCoord,
    keep_resident: &HashSet<ChunkCoord>,
    desired_load: &HashSet<ChunkCoord>,
) -> bool {
    !keep_resident.contains(&coord) || !desired_load.contains(&coord)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::asset::ManifestChunk;
    use crate::world::{ChunkData, Heightfield, WorldConfig};

    fn catalog_with_chunks(coords: &[(i32, i32)]) -> TerrainWorldCatalog {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        let dir = std::env::temp_dir().join(format!(
            "chasma_stream_cat_{}_{}",
            std::process::id(),
            id
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let entries: Vec<_> = coords
            .iter()
            .map(|&(x, z)| ManifestChunk::at(x, z, format!("chunks/{x}_{z}.ron")))
            .collect();
        let cfg = WorldConfig::default();
        let manifest = crate::terrain::asset::Manifest {
            version: crate::terrain::asset::MANIFEST_FORMAT_VERSION,
            config: crate::terrain::asset::ManifestConfig {
                chunk_size_meters: cfg.chunk_size_meters,
                units_per_meter: cfg.units_per_meter,
                meters_per_sample: cfg.meters_per_sample,
            },
            chunks: entries,
        };
        std::fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();
        TerrainWorldCatalog::from_manifest(&dir.join("manifest.ron"), &cfg).unwrap()
    }

    fn empty_chunk() -> ChunkData {
        ChunkData::new(
            Heightfield::from_samples(2, 1.0, vec![0.0; 4]).unwrap(),
            Vec::new(),
        )
    }

    fn settings(load: i32, unload: i32) -> TerrainStreamingSettings {
        TerrainStreamingSettings {
            load_radius_chunks: load,
            unload_radius_chunks: unload,
            max_loads_per_frame: 16,
            max_unloads_per_frame: 16,
            max_apply_per_frame: 16,
            max_decode_per_frame: 16,
        }
    }

    fn no_exempt() -> HashSet<ChunkId> {
        HashSet::new()
    }

    #[test]
    fn desired_set_is_local_ring_not_full_manifest() {
        let mut coords = Vec::new();
        for z in 0..20 {
            for x in 0..20 {
                coords.push((x, z));
            }
        }
        let catalog = catalog_with_chunks(&coords);
        let focus = ChunkCoord::new(10, 10);
        let desired = chunks_in_radius(focus, 1, &catalog);
        assert_eq!(desired.len(), 9);
        assert!(desired.contains(&ChunkCoord::new(10, 10)));
        assert!(!desired.contains(&ChunkCoord::new(0, 0)));
    }

    #[test]
    fn chunk_outside_residency_sets_detects_keep_and_desired_violations() {
        let mut keep = HashSet::new();
        keep.insert(ChunkCoord::new(0, 0));
        keep.insert(ChunkCoord::new(2, 0));

        let mut desired = HashSet::new();
        desired.insert(ChunkCoord::new(0, 0));

        assert!(!chunk_outside_residency_sets(
            ChunkCoord::new(0, 0),
            &keep,
            &desired
        ));
        assert!(chunk_outside_residency_sets(
            ChunkCoord::new(5, 0),
            &keep,
            &desired
        ));
        assert!(chunk_outside_residency_sets(
            ChunkCoord::new(2, 0),
            &keep,
            &desired
        ));
    }

    #[test]
    fn hysteresis_keeps_chunk_in_band_resident() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (2, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());
        world.insert(ChunkId::new(ChunkCoord::new(2, 0)), empty_chunk());

        // load=1, unload=2: chunk (2,0) is distance 2 from focus (0,0) — in keep
        // band but outside load radius; must not unload.
        let focus = PrimaryViewFocus::new(Vec3::new(128.0, 0.0, 0.0));
        let (to_load, to_unload) = diff_streaming_residency(
            &focus,
            layout,
            &settings(1, 2),
            &catalog,
            &world,
            &no_exempt(),
        );
        assert!(!to_unload.iter().any(|id| id.coord() == ChunkCoord::new(2, 0)));
        assert!(!to_load.contains(&ChunkCoord::new(2, 0)));
    }

    #[test]
    fn stationary_focus_converges_to_no_work() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (0, 1), (1, 1)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());

        let focus = PrimaryViewFocus::new(Vec3::new(256.0, 0.0, 128.0));
        let cfg = settings(1, 2);

        for _ in 0..8 {
            let (to_load, to_unload) =
                diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
            for id in to_unload {
                world.remove(id);
            }
            for coord in to_load {
                world.insert(ChunkId::new(coord), empty_chunk());
            }
        }

        let (to_load, to_unload) =
            diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
        assert!(to_load.is_empty(), "expected no loads when stable");
        assert!(to_unload.is_empty(), "expected no unloads when stable");
    }

    #[test]
    fn no_chunk_in_both_to_load_and_to_unload() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());
        world.insert(ChunkId::new(ChunkCoord::new(2, 0)), empty_chunk());
        world.insert(ChunkId::new(ChunkCoord::new(3, 0)), empty_chunk());

        let focus = PrimaryViewFocus::new(Vec3::new(128.0, 0.0, 0.0));
        let (to_load, to_unload) = diff_streaming_residency(
            &focus,
            layout,
            &settings(1, 2),
            &catalog,
            &world,
            &no_exempt(),
        );

        let unload_coords: HashSet<_> = to_unload.iter().map(|id| id.coord()).collect();
        for coord in to_load {
            assert!(
                !unload_coords.contains(&coord),
                "chunk ({}, {}) scheduled for both load and unload",
                coord.x,
                coord.z
            );
        }
    }

    #[test]
    fn boundary_chunk_does_not_alternate_when_stationary() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (2, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());

        let focus = PrimaryViewFocus::new(Vec3::new(256.0, 0.0, 0.0));
        let cfg = settings(1, 2);

        for _ in 0..6 {
            let (to_load, to_unload) =
                diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
            for id in to_unload {
                world.remove(id);
            }
            for coord in to_load {
                world.insert(ChunkId::new(coord), empty_chunk());
            }
        }

        let before = world.len();
        let (to_load, to_unload) =
            diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
        assert!(to_load.is_empty());
        assert!(to_unload.is_empty());
        assert_eq!(world.len(), before);
    }

    #[test]
    fn diff_reports_load_and_unload_candidates() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (3, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());
        world.insert(ChunkId::new(ChunkCoord::new(3, 0)), empty_chunk());

        let focus = PrimaryViewFocus::new(Vec3::new(128.0, 0.0, 0.0));
        let (to_load, to_unload) = diff_streaming_residency(
            &focus,
            layout,
            &settings(1, 2),
            &catalog,
            &world,
            &no_exempt(),
        );

        assert!(to_load.contains(&ChunkCoord::new(1, 0)));
        assert_eq!(to_unload, vec![ChunkId::new(ChunkCoord::new(3, 0))]);
    }

    #[test]
    fn hysteresis_prevents_border_oscillation() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());
        world.insert(ChunkId::new(ChunkCoord::new(2, 0)), empty_chunk());

        let focus = PrimaryViewFocus::new(Vec3::new(128.0, 0.0, 0.0));
        let cfg = settings(1, 2);

        for _ in 0..12 {
            let (to_load, to_unload) =
                diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
            for id in to_unload {
                world.remove(id);
            }
            for coord in to_load {
                world.insert(ChunkId::new(coord), empty_chunk());
            }
        }

        let (to_load, to_unload) =
            diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
        assert!(to_load.is_empty());
        assert!(to_unload.is_empty());
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(2, 0))));
    }

    #[test]
    fn no_load_unload_ping_pong_when_stationary() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (0, 1), (1, 1), (2, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());

        let focus = PrimaryViewFocus::new(Vec3::new(256.0, 0.0, 128.0));
        let cfg = settings(1, 2);
        let mut history: Vec<(usize, usize)> = Vec::new();

        for _ in 0..20 {
            let (to_load, to_unload) =
                diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
            history.push((to_load.len(), to_unload.len()));
            for id in to_unload {
                world.remove(id);
            }
            for coord in to_load {
                world.insert(ChunkId::new(coord), empty_chunk());
            }
        }

        let (to_load, to_unload) =
            diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
        assert!(to_load.is_empty());
        assert!(to_unload.is_empty());
        assert!(
            history.iter().all(|&(loads, unloads)| !(loads > 0 && unloads > 0)),
            "load and unload must not alternate in the same frame"
        );
    }

    #[test]
    fn unload_exempt_prevents_same_frame_eviction() {
        let catalog = catalog_with_chunks(&[(0, 0), (3, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());
        let chunk_id = ChunkId::new(ChunkCoord::new(3, 0));
        world.insert(chunk_id, empty_chunk());

        let focus = PrimaryViewFocus::new(Vec3::new(128.0, 0.0, 0.0));
        let mut exempt = HashSet::new();
        exempt.insert(chunk_id);

        let (_, to_unload) = diff_streaming_residency(
            &focus,
            layout,
            &settings(1, 2),
            &catalog,
            &world,
            &exempt,
        );
        assert!(!to_unload.contains(&chunk_id));
    }

    #[test]
    fn streaming_is_frame_stable_when_stationary() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (0, 1), (1, 1), (2, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(catalog.authored_extent());
        world.insert(ChunkId::new(ChunkCoord::new(1, 0)), empty_chunk());

        let focus = PrimaryViewFocus::new(Vec3::new(256.0, 0.0, 128.0));
        let cfg = settings(1, 2);

        let baseline =
            diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());

        for _ in 0..64 {
            let current =
                diff_streaming_residency(&focus, layout, &cfg, &catalog, &world, &no_exempt());
            assert_eq!(current, baseline);
        }
    }

    #[test]
    fn deterministic_residency_diff_for_identical_focus() {
        let catalog = catalog_with_chunks(&[(0, 0), (1, 0), (2, 0)]);
        let layout = WorldConfig::default().chunk_layout();
        let world = WorldData::new(layout);
        let cfg = settings(1, 2);

        let near_boundary_a = PrimaryViewFocus::new(Vec3::new(255.999, 0.0, 128.0));
        let near_boundary_b = PrimaryViewFocus::new(Vec3::new(256.001, 0.0, 128.0));

        let diff_a = diff_streaming_residency(
            &near_boundary_a,
            layout,
            &cfg,
            &catalog,
            &world,
            &no_exempt(),
        );
        let diff_b = diff_streaming_residency(
            &near_boundary_b,
            layout,
            &cfg,
            &catalog,
            &world,
            &no_exempt(),
        );
        assert_eq!(diff_a, diff_b);
        assert_eq!(
            stable_focus_chunk(near_boundary_a.position, layout),
            stable_focus_chunk(near_boundary_b.position, layout)
        );
    }
}
