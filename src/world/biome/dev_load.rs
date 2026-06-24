//! Dev startup biome mask import into [`WorldData`] (ADR-024, Phase R3).
//!
//! World-data only: loads `{source_world}/biome_mask.png` during dev preview
//! startup so [`WorldData::biome_at`] is usable. No ECS ownership or runtime copies.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::id::BiomeId;
use super::import::import_biome_mask_from_png;
use super::mapping::BiomeColorMapping;
use super::mask::BiomeMaskBounds;
use super::BiomeImportError;
use crate::logging::{append_log_line, DEV_STARTUP_LOG_PATH};
use crate::world::{ChunkExtent, WorldConfig, WorldData};

const SESSION_HEADER: &str = "# chasma dev startup log";

/// Source world folder for dev preview / manual import (`source_data/test`).
pub const DEV_SOURCE_WORLD_DIR: &str = "source_data/test";

/// Biome mask PNG at the root of [`DEV_SOURCE_WORLD_DIR`].
pub const DEV_BIOME_MASK_PATH: &str = "source_data/test/biome_mask.png";

/// Path to `biome_mask.png` inside a source world directory.
pub fn biome_mask_path_for_world(world_dir: impl AsRef<Path>) -> PathBuf {
    world_dir.as_ref().join("biome_mask.png")
}

/// Outcome of [`try_load_dev_biome_mask`].
#[derive(Debug, Clone, PartialEq)]
pub enum DevBiomeLoadOutcome {
    Loaded(DevBiomeLoadSummary),
    Missing,
    NoAuthoredExtent,
    ImportFailed(BiomeImportError),
}

/// Summary emitted after a successful dev biome import.
#[derive(Debug, Clone, PartialEq)]
pub struct DevBiomeLoadSummary {
    pub width: u32,
    pub height: u32,
    pub bounds: BiomeMaskBounds,
    pub counts: BTreeMap<BiomeId, u32>,
}

impl DevBiomeLoadSummary {
    pub fn format_counts(&self) -> String {
        let mut parts = Vec::new();
        for biome in BiomeId::all_assigned() {
            let count = self.counts.get(biome).copied().unwrap_or(0);
            parts.push(format!("{biome:?}={count}"));
        }
        if let Some(unassigned) = self.counts.get(&BiomeId::Unassigned).copied() {
            if unassigned > 0 {
                parts.push(format!("Unassigned={unassigned}"));
            }
        }
        parts.join(" ")
    }
}

/// Compute biome mask bounds from authored world extent and layout.
pub fn dev_biome_mask_bounds(
    extent: ChunkExtent,
    config: &WorldConfig,
) -> BiomeMaskBounds {
    BiomeMaskBounds::from_chunk_extent(extent, config.chunk_layout())
}

/// Import a dev biome PNG into authoritative [`WorldData`].
///
/// Missing files and import failures do not panic. The mask is stored only on
/// success via [`WorldData::set_biome_mask`].
pub fn try_load_dev_biome_mask(
    world: &mut WorldData,
    config: &WorldConfig,
    path: impl AsRef<Path>,
) -> DevBiomeLoadOutcome {
    let path = path.as_ref();
    if !path.exists() {
        return DevBiomeLoadOutcome::Missing;
    }

    let Some(extent) = world.extent() else {
        return DevBiomeLoadOutcome::NoAuthoredExtent;
    };

    let bounds = dev_biome_mask_bounds(extent, config);
    match import_biome_mask_from_png(path, bounds, &BiomeColorMapping::starter()) {
        Ok(mask) => {
            let summary = DevBiomeLoadSummary {
                width: mask.width(),
                height: mask.height(),
                bounds: mask.bounds(),
                counts: mask.biome_pixel_counts(),
            };
            world.set_biome_mask(mask);
            DevBiomeLoadOutcome::Loaded(summary)
        }
        Err(err) => DevBiomeLoadOutcome::ImportFailed(err),
    }
}

/// Load from the fixed dev path [`DEV_BIOME_MASK_PATH`].
pub fn try_load_default_dev_biome_mask(
    world: &mut WorldData,
    config: &WorldConfig,
) -> DevBiomeLoadOutcome {
    try_load_dev_biome_mask(world, config, Path::new(DEV_BIOME_MASK_PATH))
}

/// Log the dev biome load outcome to [`DEV_STARTUP_LOG_PATH`].
pub fn log_dev_biome_load_outcome(outcome: &DevBiomeLoadOutcome) {
    match outcome {
        DevBiomeLoadOutcome::Loaded(summary) => {
            let bounds = summary.bounds;
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Biome mask loaded: size={}x{} bounds=({:.0},{:.0})-({:.0},{:.0}) {}",
                    summary.width,
                    summary.height,
                    bounds.origin_x,
                    bounds.origin_z,
                    bounds.max_x(),
                    bounds.max_z(),
                    summary.format_counts(),
                ),
            );
        }
        DevBiomeLoadOutcome::Missing => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "dev biome mask not found at {DEV_BIOME_MASK_PATH}; continuing without biome data"
                ),
            );
        }
        DevBiomeLoadOutcome::NoAuthoredExtent => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                "dev biome mask skipped: WorldData authored extent not set yet",
            );
        }
        DevBiomeLoadOutcome::ImportFailed(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!("dev biome mask import failed: {err}; continuing without biome data"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::world::{ChunkCoord, LocalPosition, WorldPosition};

    fn layout() -> crate::world::ChunkLayout {
        crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn test_extent() -> ChunkExtent {
        ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        }
    }

    fn write_test_png(path: &Path, pixels: &[(u8, u8, u8)], width: u32, height: u32) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, width, height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let data: Vec<u8> = pixels
                .iter()
                .flat_map(|(r, g, b)| [*r, *g, *b])
                .collect();
            writer.write_image_data(&data).unwrap();
        }
        std::fs::write(path, buf).unwrap();
    }

    fn temp_mask_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("chasma_biome_dev_load_{name}.png"))
    }

    #[test]
    fn missing_file_handled_safely() {
        let mut world = WorldData::new(layout());
        world.set_authored_extent(test_extent());
        let config = WorldConfig::default();
        let path = temp_mask_path("missing");

        let _ = std::fs::remove_file(&path);
        let outcome = try_load_dev_biome_mask(&mut world, &config, &path);

        assert_eq!(outcome, DevBiomeLoadOutcome::Missing);
        assert!(world.biome_mask().is_none());
    }

    #[test]
    fn successful_load_stores_mask_on_world_data() {
        let path = temp_mask_path("forest");
        write_test_png(&path, &[(0, 255, 0); 4], 2, 2);

        let mut world = WorldData::new(layout());
        world.set_authored_extent(test_extent());
        let config = WorldConfig::default();

        let outcome = try_load_dev_biome_mask(&mut world, &config, &path);
        let DevBiomeLoadOutcome::Loaded(summary) = outcome else {
            panic!("expected loaded outcome, got {outcome:?}");
        };

        assert_eq!(summary.width, 2);
        assert_eq!(summary.height, 2);
        assert_eq!(summary.counts.get(&BiomeId::Forest), Some(&4));
        assert!(world.biome_mask().is_some());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn startup_without_mask_continues_when_file_missing() {
        let mut world = WorldData::new(layout());
        world.set_authored_extent(test_extent());
        let config = WorldConfig::default();
        let path = temp_mask_path("absent");
        let _ = std::fs::remove_file(&path);

        let outcome = try_load_dev_biome_mask(&mut world, &config, &path);

        assert!(matches!(outcome, DevBiomeLoadOutcome::Missing));
        assert!(world.biome_mask().is_none());
    }

    #[test]
    fn biome_lookup_works_after_startup_load() {
        let path = temp_mask_path("lookup");
        write_test_png(
            &path,
            &[(0, 255, 0), (255, 0, 0)],
            2,
            1,
        );

        let mut world = WorldData::new(layout());
        world.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(0, 0),
        });
        let config = WorldConfig::default();
        assert!(matches!(
            try_load_dev_biome_mask(&mut world, &config, &path),
            DevBiomeLoadOutcome::Loaded(_)
        ));

        let forest = world
            .biome_at(WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(64.0, 0.0, 128.0)),
            ))
            .unwrap();
        assert_eq!(forest.biome, BiomeId::Forest);

        let desert = world
            .biome_at(WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(200.0, 0.0, 128.0)),
            ))
            .unwrap();
        assert_eq!(desert.biome, BiomeId::Desert);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn import_failure_does_not_store_mask() {
        let path = temp_mask_path("invalid");
        std::fs::write(&path, b"not a png").unwrap();

        let mut world = WorldData::new(layout());
        world.set_authored_extent(test_extent());
        let config = WorldConfig::default();

        let outcome = try_load_dev_biome_mask(&mut world, &config, &path);

        assert!(matches!(outcome, DevBiomeLoadOutcome::ImportFailed(_)));
        assert!(world.biome_mask().is_none());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn no_authored_extent_skips_load() {
        let path = temp_mask_path("no_extent");
        write_test_png(&path, &[(0, 255, 0)], 1, 1);

        let mut world = WorldData::new(layout());
        let config = WorldConfig::default();

        assert_eq!(
            try_load_dev_biome_mask(&mut world, &config, &path),
            DevBiomeLoadOutcome::NoAuthoredExtent
        );
        assert!(world.biome_mask().is_none());

        let _ = std::fs::remove_file(path);
    }
}
