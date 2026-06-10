//! Offline EXR heightfield decoding (ADR-003, ADR-009).
//!
//! This module decodes an externally-authored OpenEXR heightfield into raw
//! `f32` samples wrapped in a [`SourceHeightfield`] — the input to the
//! deterministic partitioner ([`import_world`](super::import_world)).
//!
//! Per ADR-009 (Phase 1B addendum) this is an *offline / preprocessing* tool:
//! the runtime loads pre-chunked terrain assets, not monolithic source
//! heightfields. The decoder turns authored monolithic data into samples the
//! partitioner can split into per-chunk tiles; it is deliberately not wired as a
//! runtime startup system.
//!
//! The heightfield is decoded to plain `f32` and is never routed through a Bevy
//! `Image`/texture, preserving the authoritative-data boundary (ADR-003).

use std::path::Path;

use exr::prelude::{FlatSamples, read_first_flat_layer_from_file};

use super::{ImportError, SourceHeightfield};

/// Errors produced while decoding an EXR heightfield file.
///
/// File and format failures are distinct from the partition-time failures in
/// [`ImportError`]. Once decoding succeeds, sample-level validation (finite
/// values, dimensions) is delegated to [`SourceHeightfield::from_samples`] and
/// surfaced via [`DecodeError::Source`].
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// The `exr` crate failed to open or parse the file.
    Exr(String),
    /// The decoded layer contained no channels.
    NoChannels,
    /// The height channel used an unsupported sample format. Only floating-point
    /// samples are valid heights; integer EXR data is rejected (ADR-003).
    UnsupportedSampleFormat,
    /// The image dimensions did not fit the source heightfield's `u32` extents.
    DimensionTooLarge { width: usize, height: usize },
    /// Sample-level validation failed (non-finite values, dimension mismatch).
    Source(ImportError),
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Exr(msg) => write!(f, "failed to decode EXR heightfield: {msg}"),
            Self::NoChannels => write!(f, "EXR layer contained no channels"),
            Self::UnsupportedSampleFormat => {
                write!(f, "EXR height channel is not a floating-point channel")
            }
            Self::DimensionTooLarge { width, height } => write!(
                f,
                "EXR dimensions {width}x{height} exceed the supported source size"
            ),
            Self::Source(err) => write!(f, "decoded EXR produced invalid samples: {err}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decode an OpenEXR heightfield file into a [`SourceHeightfield`] (ADR-009).
///
/// The heightfield is expected to be a single-channel (or grayscale)
/// floating-point image; the decoder reads the layer's first channel as height.
/// EXR stores samples top-to-bottom, row by row, which maps directly onto the
/// source-grid convention that row 0 is the minimum-`Z` edge (ADR-009 addendum).
///
/// Half-float (`f16`) channels are promoted to `f32`; integer channels are
/// rejected. Finiteness and dimension validation are performed by
/// [`SourceHeightfield::from_samples`].
pub fn decode_exr_heightfield(path: impl AsRef<Path>) -> Result<SourceHeightfield, DecodeError> {
    let image =
        read_first_flat_layer_from_file(path).map_err(|e| DecodeError::Exr(e.to_string()))?;

    let layer = image.layer_data;
    let width_usize = layer.size.0;
    let height_usize = layer.size.1;

    let channel = layer
        .channel_data
        .list
        .first()
        .ok_or(DecodeError::NoChannels)?;

    let samples: Vec<f32> = match &channel.sample_data {
        FlatSamples::F32(values) => values.clone(),
        FlatSamples::F16(values) => values.iter().map(|h| h.to_f32()).collect(),
        FlatSamples::U32(_) => return Err(DecodeError::UnsupportedSampleFormat),
    };

    let width = u32::try_from(width_usize).map_err(|_| DecodeError::DimensionTooLarge {
        width: width_usize,
        height: height_usize,
    })?;
    let height = u32::try_from(height_usize).map_err(|_| DecodeError::DimensionTooLarge {
        width: width_usize,
        height: height_usize,
    })?;

    SourceHeightfield::from_samples(width, height, samples).map_err(DecodeError::Source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkId, WorldConfig, import_world};
    use exr::prelude::write_rgb_file;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A unique temp path per call so parallel tests do not collide.
    fn temp_exr_path() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("chasma_decode_{pid}_{n}.exr"))
    }

    /// Write a `width` x `height` EXR where each pixel encodes `col + row * 100`
    /// identically in all three channels, making provenance easy to assert.
    fn write_synthetic_exr(path: &std::path::Path, width: usize, height: usize) {
        write_rgb_file(path, width, height, |x, y| {
            let v = x as f32 + y as f32 * 100.0;
            (v, v, v)
        })
        .unwrap();
    }

    #[test]
    fn decodes_dimensions_and_samples_in_row_major_top_to_bottom() {
        let path = temp_exr_path();
        write_synthetic_exr(&path, 3, 2);

        let decoded = decode_exr_heightfield(&path).unwrap();

        let mut expected_samples = Vec::new();
        for row in 0..2u32 {
            for col in 0..3u32 {
                expected_samples.push(col as f32 + row as f32 * 100.0);
            }
        }
        let expected = SourceHeightfield::from_samples(3, 2, expected_samples).unwrap();
        assert_eq!(decoded, expected);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn decoded_source_partitions_into_world_data() {
        let path = temp_exr_path();
        write_synthetic_exr(&path, 5, 3);

        let decoded = decode_exr_heightfield(&path).unwrap();
        let config = WorldConfig {
            chunk_size_meters: 2.0,
            units_per_meter: 1.0,
            meters_per_sample: 1.0,
        };
        let world = import_world(&decoded, &config).unwrap();

        assert_eq!(world.len(), 2);
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(0, 0))));
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(1, 0))));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reports_error_for_missing_file() {
        let path = temp_exr_path();
        let err = decode_exr_heightfield(&path).unwrap_err();
        assert!(matches!(err, DecodeError::Exr(_)));
    }
}
