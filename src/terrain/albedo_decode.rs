//! Albedo sidecar decoding (ADR-009, ADR-011).
//!
//! Decodes optional per-chunk albedo files into [`ChunkAlbedoGrid`]. File IO
//! lives here; RON text decoding is delivery-agnostic like height chunks.

use std::path::{Path, PathBuf};

use super::albedo::ChunkAlbedoGrid;
use super::asset::{ALBEDO_FORMAT_VERSION, AlbedoFile, TerrainAssetError};

/// Raw albedo sidecar bytes read during the IO stage (decode deferred until height is known).
#[derive(Debug, Clone)]
pub struct AlbedoSidecarIo {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// Decode an albedo sidecar from RON text.
pub fn decode_albedo_ron(text: &str) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    let file: AlbedoFile =
        ron::from_str(text).map_err(|err| TerrainAssetError::Ron(err.to_string()))?;
    if file.version != ALBEDO_FORMAT_VERSION {
        return Err(TerrainAssetError::UnsupportedVersion {
            found: file.version,
            expected: ALBEDO_FORMAT_VERSION,
        });
    }
    ChunkAlbedoGrid::from_samples(file.samples_per_edge as usize, file.samples)
        .map_err(TerrainAssetError::AlbedoGrid)
}

fn albedo_extension(path: &Path) -> &str {
    path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
}

/// Decode an albedo sidecar from in-memory bytes (extension selects the decoder).
pub fn decode_albedo_from_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    match albedo_extension(path) {
        "ron" => {
            let text = std::str::from_utf8(bytes).map_err(|err| TerrainAssetError::AlbedoDecode {
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
            decode_albedo_ron(text)
        }
        "exr" => decode_albedo_exr_bytes(path, bytes),
        "png" => decode_albedo_png_bytes(path, bytes),
        other => Err(TerrainAssetError::AlbedoUnsupportedFormat {
            path: path.display().to_string(),
            extension: other.to_string(),
        }),
    }
}

/// Decode an albedo sidecar from disk, choosing the decoder from the extension.
pub fn decode_albedo_from_path(path: &Path) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    let bytes = std::fs::read(path).map_err(|err| TerrainAssetError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;
    decode_albedo_from_bytes(path, &bytes)
}

/// Decode an OpenEXR RGB albedo tile (`Albedo_y{z}_x{x}.exr` or runtime sidecar).
#[cfg(feature = "terrain-import")]
pub fn decode_albedo_exr(path: impl AsRef<Path>) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    use exr::prelude::{FlatSamples, read_first_flat_layer_from_file};

    let path = path.as_ref();
    let image = read_first_flat_layer_from_file(path)
        .map_err(|err| TerrainAssetError::AlbedoDecode {
            path: path.display().to_string(),
            message: err.to_string(),
        })?;

    let layer = image.layer_data;
    let width = layer.size.0;
    let height = layer.size.1;
    if width != height {
        return Err(TerrainAssetError::AlbedoDimensionMismatch {
            path: path.display().to_string(),
            width,
            height,
            expected_samples_per_edge: width,
        });
    }

    let channels = &layer.channel_data.list;
    if channels.is_empty() {
        return Err(TerrainAssetError::AlbedoDecode {
            path: path.display().to_string(),
            message: "EXR layer contained no channels".to_string(),
        });
    }

    let read_named_channel = |name: &str| -> Result<Vec<f32>, TerrainAssetError> {
        let channel = channels
            .iter()
            .find(|ch| ch.name.to_string() == name)
            .ok_or_else(|| TerrainAssetError::AlbedoDecode {
                path: path.display().to_string(),
                message: format!(
                    "missing '{name}' channel (found: {})",
                    channels
                        .iter()
                        .map(|ch| ch.name.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            })?;

        match &channel.sample_data {
            FlatSamples::F32(values) => Ok(values.clone()),
            FlatSamples::F16(values) => Ok(values.iter().map(|h| h.to_f32()).collect()),
            FlatSamples::U32(_) => Err(TerrainAssetError::AlbedoDecode {
                path: path.display().to_string(),
                message: format!("integer EXR channel '{name}' is not supported for albedo"),
            }),
        }
    };

    let r = read_named_channel("R")?;
    let g = read_named_channel("G")?;
    let b = read_named_channel("B")?;
    if r.len() != g.len() || g.len() != b.len() {
        return Err(TerrainAssetError::AlbedoDecode {
            path: path.display().to_string(),
            message: "RGB channel lengths differ".to_string(),
        });
    }

    let data: Vec<[f32; 3]> = r
        .into_iter()
        .zip(g)
        .zip(b)
        .map(|((rv, gv), bv)| [rv, gv, bv])
        .collect();

    ChunkAlbedoGrid::from_samples(width, data).map_err(TerrainAssetError::AlbedoGrid)
}

#[cfg(not(feature = "terrain-import"))]
pub fn decode_albedo_exr(path: impl AsRef<Path>) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    Err(TerrainAssetError::AlbedoUnsupportedFormat {
        path: path.as_ref().display().to_string(),
        extension: "exr".to_string(),
    })
}

/// Decode an 8-bit PNG albedo tile (`Albedo_y{z}_x{x}.png` or runtime sidecar).
#[cfg(feature = "terrain-import")]
pub fn decode_albedo_png(path: impl AsRef<Path>) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    use std::io::BufReader;

    let path = path.as_ref();
    let file = std::fs::File::open(path).map_err(|err| TerrainAssetError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().map_err(|err| TerrainAssetError::AlbedoDecode {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;

    let width = reader.info().width as usize;
    let height = reader.info().height as usize;
    if width != height {
        return Err(TerrainAssetError::AlbedoDimensionMismatch {
            path: path.display().to_string(),
            width,
            height,
            expected_samples_per_edge: width,
        });
    }

    let buf_size = reader.output_buffer_size();
    let mut buf = vec![0u8; buf_size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|err| TerrainAssetError::AlbedoDecode {
            path: path.display().to_string(),
            message: err.to_string(),
        })?;

    let pixels = match info.color_type {
        png::ColorType::Rgb => 3usize,
        png::ColorType::Rgba => 4usize,
        other => {
            return Err(TerrainAssetError::AlbedoDecode {
                path: path.display().to_string(),
                message: format!("unsupported PNG color type: {other:?}"),
            });
        }
    };

    let data: Vec<[f32; 3]> = buf
        .chunks_exact(pixels)
        .map(|px| [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0])
        .collect();

    ChunkAlbedoGrid::from_samples(width, data).map_err(TerrainAssetError::AlbedoGrid)
}

#[cfg(not(feature = "terrain-import"))]
pub fn decode_albedo_png(path: impl AsRef<Path>) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    Err(TerrainAssetError::AlbedoUnsupportedFormat {
        path: path.as_ref().display().to_string(),
        extension: "png".to_string(),
    })
}

/// Decode PNG albedo bytes (IO stage reads; compute stage decodes).
#[cfg(feature = "terrain-import")]
fn decode_albedo_png_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    use std::io::Cursor;

    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|err| TerrainAssetError::AlbedoDecode {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;

    let width = reader.info().width as usize;
    let height = reader.info().height as usize;
    if width != height {
        return Err(TerrainAssetError::AlbedoDimensionMismatch {
            path: path.display().to_string(),
            width,
            height,
            expected_samples_per_edge: width,
        });
    }

    let buf_size = reader.output_buffer_size();
    let mut buf = vec![0u8; buf_size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|err| TerrainAssetError::AlbedoDecode {
            path: path.display().to_string(),
            message: err.to_string(),
        })?;

    let pixels = match info.color_type {
        png::ColorType::Rgb => 3usize,
        png::ColorType::Rgba => 4usize,
        other => {
            return Err(TerrainAssetError::AlbedoDecode {
                path: path.display().to_string(),
                message: format!("unsupported PNG color type: {other:?}"),
            });
        }
    };

    let data: Vec<[f32; 3]> = buf
        .chunks_exact(pixels)
        .map(|px| [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0])
        .collect();

    ChunkAlbedoGrid::from_samples(width, data).map_err(TerrainAssetError::AlbedoGrid)
}

#[cfg(not(feature = "terrain-import"))]
fn decode_albedo_png_bytes(
    path: &Path,
    _bytes: &[u8],
) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    Err(TerrainAssetError::AlbedoUnsupportedFormat {
        path: path.display().to_string(),
        extension: "png".to_string(),
    })
}

/// Decode EXR albedo bytes (falls back to a temp file when no in-memory reader exists).
#[cfg(feature = "terrain-import")]
fn decode_albedo_exr_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    use std::io::Write;

    let temp = std::env::temp_dir().join(format!(
        "chasma_albedo_exr_{}_{}",
        std::process::id(),
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("sidecar.exr")
    ));
    {
        let mut file = std::fs::File::create(&temp).map_err(|err| TerrainAssetError::Io {
            path: temp.display().to_string(),
            message: err.to_string(),
        })?;
        file.write_all(bytes).map_err(|err| TerrainAssetError::Io {
            path: temp.display().to_string(),
            message: err.to_string(),
        })?;
    }
    let result = decode_albedo_exr(&temp);
    std::fs::remove_file(&temp).ok();
    result
}

#[cfg(not(feature = "terrain-import"))]
fn decode_albedo_exr_bytes(
    path: &Path,
    _bytes: &[u8],
) -> Result<ChunkAlbedoGrid, TerrainAssetError> {
    Err(TerrainAssetError::AlbedoUnsupportedFormat {
        path: path.display().to_string(),
        extension: "exr".to_string(),
    })
}

/// Read optional albedo sidecar bytes on the IO pool (no decode).
pub fn read_albedo_sidecar_bytes(path: &Path) -> Result<Option<AlbedoSidecarIo>, TerrainAssetError> {
    if !path.is_file() {
        eprintln!(
            "terrain albedo: sidecar missing at {}, continuing without albedo",
            path.display()
        );
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|err| TerrainAssetError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;
    Ok(Some(AlbedoSidecarIo {
        path: path.to_path_buf(),
        bytes,
    }))
}

/// Decode and validate albedo sidecar bytes once height sample resolution is known.
pub fn decode_albedo_sidecar_io(
    sidecar: &AlbedoSidecarIo,
    height_samples_per_edge: u32,
) -> Result<Option<ChunkAlbedoGrid>, TerrainAssetError> {
    let albedo = decode_albedo_from_bytes(&sidecar.path, &sidecar.bytes)?;
    validate_albedo_for_height(&albedo, height_samples_per_edge, &sidecar.path)?;
    Ok(Some(albedo))
}

/// Validate decoded albedo against the height chunk grid.
pub fn validate_albedo_for_height(
    albedo: &ChunkAlbedoGrid,
    height_samples_per_edge: u32,
    path: &Path,
) -> Result<(), TerrainAssetError> {
    if albedo.matches_height_samples(height_samples_per_edge) {
        Ok(())
    } else {
        Err(TerrainAssetError::AlbedoDimensionMismatch {
            path: path.display().to_string(),
            width: albedo.samples_per_edge,
            height: albedo.samples_per_edge,
            expected_samples_per_edge: height_samples_per_edge as usize,
        })
    }
}

/// Load an optional albedo sidecar from an absolute path.
///
/// Missing files log a warning and return `Ok(None)`. Present files must match
/// the height grid or return an error.
pub fn load_albedo_sidecar_absolute(
    path: &Path,
    height_samples_per_edge: u32,
) -> Result<Option<ChunkAlbedoGrid>, TerrainAssetError> {
    let Some(sidecar) = read_albedo_sidecar_bytes(path)? else {
        return Ok(None);
    };
    decode_albedo_sidecar_io(&sidecar, height_samples_per_edge)
}

/// Load optional albedo for a manifest entry.
///
/// - No `albedo_path` → `Ok(None)`.
/// - Missing file on disk → warning log + `Ok(None)`.
/// - Present file with dimension mismatch → error.
pub fn try_load_optional_albedo(
    base_dir: &Path,
    albedo_path: Option<&str>,
    height_samples_per_edge: u32,
) -> Result<Option<ChunkAlbedoGrid>, TerrainAssetError> {
    let Some(rel) = albedo_path else {
        return Ok(None);
    };

    let path = base_dir.join(rel);
    if !path.is_file() {
        eprintln!(
            "terrain albedo: sidecar missing at {}, continuing without albedo",
            path.display()
        );
        return Ok(None);
    }

    let albedo = decode_albedo_from_path(&path)?;
    validate_albedo_for_height(&albedo, height_samples_per_edge, &path)?;
    Ok(Some(albedo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::asset::ALBEDO_FORMAT_VERSION;

    fn sample_albedo_file() -> AlbedoFile {
        AlbedoFile {
            version: ALBEDO_FORMAT_VERSION,
            samples_per_edge: 3,
            samples: vec![[0.1, 0.2, 0.3]; 9],
        }
    }

    #[test]
    fn decode_albedo_ron_round_trip() {
        let text = ron::to_string(&sample_albedo_file()).unwrap();
        let grid = decode_albedo_ron(&text).unwrap();
        assert_eq!(grid.samples_per_edge, 3);
        assert_eq!(grid.data.len(), 9);
        assert_eq!(grid.data[0], [0.1, 0.2, 0.3]);
    }

    #[test]
    fn validate_rejects_dimension_mismatch() {
        let grid = ChunkAlbedoGrid::from_samples(2, vec![[1.0; 3]; 4]).unwrap();
        let err = validate_albedo_for_height(&grid, 3, Path::new("test.albedo.ron")).unwrap_err();
        assert!(matches!(
            err,
            TerrainAssetError::AlbedoDimensionMismatch {
                expected_samples_per_edge: 3,
                ..
            }
        ));
    }

    #[test]
    #[cfg(feature = "terrain-import")]
    fn decode_albedo_exr_reads_channels_by_name_not_alphabetical_index() {
        use exr::prelude::write_rgb_file;
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("chasma_albedo_bgr_{n}.exr"));

        write_rgb_file(&path, 2, 2, |_, _| (0.9, 0.2, 0.1)).unwrap();

        let grid = decode_albedo_exr(&path).unwrap();
        assert_eq!(grid.data[0], [0.9, 0.2, 0.1]);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn missing_sidecar_returns_none() {
        let dir = std::env::temp_dir();
        let albedo = try_load_optional_albedo(&dir, Some("nope/missing.albedo.ron"), 3).unwrap();
        assert!(albedo.is_none());
    }
}
