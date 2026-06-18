//! Terrain mesh LOD selection policy (ADR-013 Phase 2C).
//!
//! Pure chunk-ring distance → [`ChunkLod`] mapping. No camera or ECS dependency.
//!
//! **LOD does not control visible terrain distance.** It only selects mesh
//! resolution for chunks that streaming has already made resident (see
//! [`super::streaming::TerrainStreamingSettings`]). Unloaded chunks never receive
//! LOD meshes.

use bevy::prelude::*;

use crate::world::ChunkCoord;

use super::catalog::TerrainWorldCatalog;
use super::mesh::ChunkLod;
use super::streaming::chunk_chebyshev_distance;

/// Tunable Chebyshev ring thresholds for mesh LOD (ADR-013 Phase 2C).
///
/// These distances are measured from the stable view focus chunk to each **resident**
/// render chunk. They affect subsampling only — not which chunks load or unload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]
#[reflect(Resource)]
pub struct TerrainLodSettings {
    /// Maximum Chebyshev distance (inclusive) from focus for [`ChunkLod::Full`].
    pub full_max_distance: i32,
    /// Maximum Chebyshev distance (inclusive) from focus for [`ChunkLod::Half`].
    pub half_max_distance: i32,
    /// Maximum Chebyshev distance (inclusive) from focus for [`ChunkLod::Quarter`].
    pub quarter_max_distance: i32,
    /// Soft cap on immediate async LOD mesh builds enqueued per frame (`0` = unlimited).
    pub max_lod_builds_per_frame: usize,
    /// Soft cap on predictive LOD prefetch builds per frame (`0` = unlimited).
    ///
    /// Budgeted separately from [`Self::max_lod_builds_per_frame`]; does not load chunks.
    pub max_lod_prefetch_per_frame: usize,
}

impl Default for TerrainLodSettings {
    fn default() -> Self {
        Self {
            // Tighter than legacy 3/8 rings: sharper near focus, Eighth from distance 5+.
            full_max_distance: 0,
            half_max_distance: 1,
            quarter_max_distance: 4,
            max_lod_builds_per_frame: 2,
            max_lod_prefetch_per_frame: 6,
        }
    }
}

/// Predictive LOD warmup band relative to streaming load radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
#[reflect(Debug, PartialEq)]
pub enum LodPriority {
    /// Inside `load_radius_chunks` — highest scheduling priority.
    High = 0,
    /// `load_radius_chunks + 1` ring.
    Medium = 1,
    /// `load_radius_chunks + 2` ring.
    Low = 2,
}

/// One finer mesh-resolution level, if any.
pub fn finer_lod(lod: ChunkLod) -> Option<ChunkLod> {
    match lod {
        ChunkLod::Eighth => Some(ChunkLod::Quarter),
        ChunkLod::Quarter => Some(ChunkLod::Half),
        ChunkLod::Half => Some(ChunkLod::Full),
        ChunkLod::Full => None,
    }
}

/// LOD level to prefetch for camera-move warmup: one step finer than display, ring-capped.
///
/// Never returns [`ChunkLod::Full`] beyond [`TerrainLodSettings::full_max_distance`].
/// Does not prefetch coarser levels or duplicate the current display LOD.
pub fn prefetch_warmup_lod(
    focus_chunk: ChunkCoord,
    chunk_coord: ChunkCoord,
    settings: &TerrainLodSettings,
) -> Option<ChunkLod> {
    let display = desired_lod(focus_chunk, chunk_coord, settings);
    let finer = finer_lod(display)?;
    if finer == ChunkLod::Full {
        let distance = chunk_chebyshev_distance(focus_chunk, chunk_coord);
        if distance > settings.full_max_distance {
            return None;
        }
    }
    Some(finer)
}

/// Classify Chebyshev distance into a prefetch band (`None` outside +2 ring).
pub fn lod_prefetch_priority(
    focus_chunk: ChunkCoord,
    chunk_coord: ChunkCoord,
    load_radius_chunks: i32,
) -> Option<LodPriority> {
    let distance = chunk_chebyshev_distance(focus_chunk, chunk_coord);
    if distance <= load_radius_chunks {
        Some(LodPriority::High)
    } else if distance == load_radius_chunks + 1 {
        Some(LodPriority::Medium)
    } else if distance == load_radius_chunks + 2 {
        Some(LodPriority::Low)
    } else {
        None
    }
}

/// Predictive LOD warmup targets for authored catalog chunks in the load-radius +2 band.
///
/// Pure, deterministic, O(r²) over the catalog neighborhood. Does **not** load chunks
/// or touch residency — callers enqueue builds only for already-resident chunks.
///
/// `load_radius_chunks` comes from [`super::streaming::TerrainStreamingSettings`] at
/// the call site (streaming radius is unchanged by prefetch).
pub fn predicted_lod_targets(
    focus_chunk: ChunkCoord,
    catalog: &TerrainWorldCatalog,
    settings: &TerrainLodSettings,
    load_radius_chunks: i32,
) -> Vec<(ChunkCoord, ChunkLod, LodPriority)> {
    let outer = load_radius_chunks + 2;
    let mut out = Vec::new();

    for dz in -outer..=outer {
        for dx in -outer..=outer {
            let coord = ChunkCoord::new(focus_chunk.x + dx, focus_chunk.z + dz);
            if !catalog.contains(coord) {
                continue;
            }
            let Some(priority) = lod_prefetch_priority(focus_chunk, coord, load_radius_chunks)
            else {
                continue;
            };
            let Some(lod) = prefetch_warmup_lod(focus_chunk, coord, settings) else {
                continue;
            };
            out.push((coord, lod, priority));
        }
    }

    out.sort_by(|a, b| {
        a.2.cmp(&b.2)
            .then_with(|| a.0.z.cmp(&b.0.z))
            .then_with(|| a.0.x.cmp(&b.0.x))
            .then_with(|| lod_order(a.1).cmp(&lod_order(b.1)))
    });
    out
}

pub(crate) fn lod_order(lod: ChunkLod) -> u8 {
    match lod {
        ChunkLod::Full => 0,
        ChunkLod::Half => 1,
        ChunkLod::Quarter => 2,
        ChunkLod::Eighth => 3,
    }
}

/// Chebyshev chunk-ring distance from `focus` → mesh LOD.
///
/// Distances above [`TerrainLodSettings::quarter_max_distance`] map to
/// [`ChunkLod::Eighth`].
pub fn desired_lod(
    focus_chunk: ChunkCoord,
    chunk_coord: ChunkCoord,
    settings: &TerrainLodSettings,
) -> ChunkLod {
    let distance = chunk_chebyshev_distance(focus_chunk, chunk_coord);
    if distance <= settings.full_max_distance {
        ChunkLod::Full
    } else if distance <= settings.half_max_distance {
        ChunkLod::Half
    } else if distance <= settings.quarter_max_distance {
        ChunkLod::Quarter
    } else {
        ChunkLod::Eighth
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::asset::{Manifest, ManifestChunk, MANIFEST_FORMAT_VERSION};
    use crate::terrain::load::config_snapshot;
    use crate::world::{ChunkCoord, WorldConfig};
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn coord(x: i32, z: i32) -> ChunkCoord {
        ChunkCoord::new(x, z)
    }

    fn test_catalog(coords: &[(i32, i32)]) -> TerrainWorldCatalog {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("chasma_lod_cat_{id}"));
        fs::create_dir_all(&dir).unwrap();
        let config = WorldConfig::default();
        let chunks: Vec<ManifestChunk> = coords
            .iter()
            .map(|(x, z)| ManifestChunk::at(*x, *z, format!("{x}_{z}.ron")))
            .collect();
        let manifest = Manifest {
            version: MANIFEST_FORMAT_VERSION,
            config: config_snapshot(&config),
            chunks,
        };
        fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();
        let catalog =
            TerrainWorldCatalog::from_manifest(&dir.join("manifest.ron"), &config).unwrap();
        fs::remove_dir_all(&dir).ok();
        catalog
    }

    #[test]
    fn desired_lod_ring_table_distances_zero_through_five() {
        let settings = TerrainLodSettings {
            full_max_distance: 0,
            half_max_distance: 1,
            quarter_max_distance: 2,
            ..Default::default()
        };
        let focus = coord(10, 10);

        assert_eq!(desired_lod(focus, coord(10, 10), &settings), ChunkLod::Full);

        for dz in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let c = coord(10 + dx, 10 + dz);
                assert_eq!(
                    desired_lod(focus, c, &settings),
                    ChunkLod::Half,
                    "distance-1 neighbor ({dx},{dz})"
                );
            }
        }

        assert_eq!(desired_lod(focus, coord(12, 10), &settings), ChunkLod::Quarter);
        assert_eq!(desired_lod(focus, coord(10, 12), &settings), ChunkLod::Quarter);
        assert_eq!(desired_lod(focus, coord(12, 12), &settings), ChunkLod::Quarter);

        assert_eq!(desired_lod(focus, coord(13, 10), &settings), ChunkLod::Eighth);
        assert_eq!(desired_lod(focus, coord(10, 13), &settings), ChunkLod::Eighth);
        assert_eq!(desired_lod(focus, coord(13, 13), &settings), ChunkLod::Eighth);
        assert_eq!(desired_lod(focus, coord(15, 10), &settings), ChunkLod::Eighth);
        assert_eq!(desired_lod(focus, coord(10, 15), &settings), ChunkLod::Eighth);
    }

    #[test]
    fn default_lod_rings_are_tight_for_distant_coarsening() {
        let settings = TerrainLodSettings::default();
        let focus = coord(10, 10);

        assert_eq!(settings.full_max_distance, 0);
        assert_eq!(settings.half_max_distance, 1);
        assert_eq!(settings.quarter_max_distance, 4);

        assert_eq!(desired_lod(focus, focus, &settings), ChunkLod::Full);
        assert_eq!(desired_lod(focus, coord(11, 10), &settings), ChunkLod::Half);
        assert_eq!(desired_lod(focus, coord(14, 10), &settings), ChunkLod::Quarter);
        assert_eq!(desired_lod(focus, coord(15, 10), &settings), ChunkLod::Eighth);
    }

    #[test]
    fn prefetch_warmup_is_one_step_finer_and_skips_distant_full() {
        let settings = TerrainLodSettings::default();
        let focus = coord(0, 0);

        assert_eq!(prefetch_warmup_lod(focus, focus, &settings), None);
        assert_eq!(
            prefetch_warmup_lod(focus, coord(1, 0), &settings),
            None,
            "Full is not prefetched beyond full_max_distance"
        );
        assert_eq!(
            prefetch_warmup_lod(focus, coord(2, 0), &settings),
            Some(ChunkLod::Half)
        );
        assert_eq!(
            prefetch_warmup_lod(focus, coord(6, 0), &settings),
            Some(ChunkLod::Quarter)
        );
    }

    #[test]
    fn predicted_lod_targets_returns_correct_prefetch_rings() {
        let settings = TerrainLodSettings::default();
        let focus = coord(10, 10);
        let load_radius = 1;
        let catalog = test_catalog(&[
            (10, 10),
            (11, 10),
            (12, 10),
            (13, 10),
            (10, 13),
        ]);

        let targets = predicted_lod_targets(focus, &catalog, &settings, load_radius);

        assert!(!targets.iter().any(|(_, lod, _)| *lod == ChunkLod::Full));
        assert!(targets.iter().any(|(c, lod, p)| {
            *c == coord(12, 10) && *lod == ChunkLod::Half && *p == LodPriority::Medium
        }));
        assert!(targets.iter().any(|(c, lod, p)| {
            *c == coord(13, 10) && *lod == ChunkLod::Half && *p == LodPriority::Low
        }));
        assert!(!targets.iter().any(|(c, _, _)| *c == coord(14, 10)));
    }

    #[test]
    fn predicted_lod_targets_is_deterministic() {
        let settings = TerrainLodSettings::default();
        let focus = coord(0, 0);
        let catalog = test_catalog(&[(0, 0), (1, 0), (2, 0), (0, 1), (0, 2)]);
        let a = predicted_lod_targets(focus, &catalog, &settings, 0);
        let b = predicted_lod_targets(focus, &catalog, &settings, 0);
        assert_eq!(a, b);
    }
}
