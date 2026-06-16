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

use super::mesh::ChunkLod;
use super::streaming::chunk_chebyshev_distance;

/// Tunable Chebyshev ring thresholds for mesh LOD (ADR-013 Phase 2C).
///
/// These distances are measured from the stable view focus chunk to each **resident**
/// render chunk. They affect subsampling only — not which chunks load or unload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]#[reflect(Resource)]
pub struct TerrainLodSettings {
    /// Maximum Chebyshev distance (inclusive) from focus for [`ChunkLod::Full`].
    pub full_max_distance: i32,
    /// Maximum Chebyshev distance (inclusive) from focus for [`ChunkLod::Half`].
    pub half_max_distance: i32,
    /// Maximum Chebyshev distance (inclusive) from focus for [`ChunkLod::Quarter`].
    pub quarter_max_distance: i32,
    /// Soft cap on new async LOD mesh builds enqueued per frame (`0` = unlimited).
    pub max_lod_builds_per_frame: usize,
}

impl Default for TerrainLodSettings {
    fn default() -> Self {
        Self {
            full_max_distance: 0,
            half_max_distance: 1,
            quarter_max_distance: 2,
            max_lod_builds_per_frame: 2,
        }
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
    use crate::world::ChunkCoord;

    fn coord(x: i32, z: i32) -> ChunkCoord {
        ChunkCoord::new(x, z)
    }

    #[test]
    fn desired_lod_ring_table_distances_zero_through_five() {
        let settings = TerrainLodSettings::default();
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
}
