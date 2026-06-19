//! World-scale biome mask authority (ADR-024).
//!
//! Biome classification is world data, not terrain geometry or runtime state.
//! This module provides PNG import, compact storage, and deterministic
//! world-position sampling.

mod error;
mod id;
mod import;
mod mapping;
mod mask;
mod sample;

#[cfg(any(test, feature = "dev"))]
mod dev_load;

pub use error::BiomeImportError;
pub use id::BiomeId;
pub use import::{import_biome_mask_from_png, import_biome_mask_from_png_bytes};
pub use mapping::{BiomeColorEntry, BiomeColorMapping};
pub use mask::{BiomeMask, BiomeMaskBounds};
pub use sample::BiomeSample;

#[cfg(any(test, feature = "dev"))]
pub use dev_load::{
    dev_biome_mask_bounds, log_dev_biome_load_outcome, try_load_default_dev_biome_mask,
    try_load_dev_biome_mask, biome_mask_path_for_world, DevBiomeLoadOutcome, DevBiomeLoadSummary,
    DEV_BIOME_MASK_PATH, DEV_SOURCE_WORLD_DIR,
};
