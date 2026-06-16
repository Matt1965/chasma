//! Delivery-agnostic decoding of pre-chunked terrain assets (ADR-011, ADR-012).
//!
//! These functions consume already-read text and never touch the filesystem, so
//! the Phase 2A synchronous loader, Phase 2B on-demand loading, and a future
//! `AssetLoader` can share the exact same decode path.

use crate::world::{ChunkCoord, ChunkData, ChunkId, Heightfield, TerrainMetadata};

#[cfg(any(test, feature = "terrain-import"))]
use super::albedo::TerrainChunkPayload;
use super::asset::{
    CHUNK_FORMAT_VERSION, ChunkFile, MANIFEST_FORMAT_VERSION, Manifest, TerrainAssetError,
};

/// Decode a manifest from RON text.
pub fn decode_manifest(text: &str) -> Result<Manifest, TerrainAssetError> {
    let manifest: Manifest =
        ron::from_str(text).map_err(|err| TerrainAssetError::Ron(err.to_string()))?;
    if manifest.version != MANIFEST_FORMAT_VERSION {
        return Err(TerrainAssetError::UnsupportedVersion {
            found: manifest.version,
            expected: MANIFEST_FORMAT_VERSION,
        });
    }
    Ok(manifest)
}

/// Decode a single chunk file from RON text into its authoritative
/// [`ChunkData`] and identity.
///
/// The heightfield is the single source of truth: metadata is recomputed from
/// the samples and the stored range is validated against it as a corruption
/// check (ADR-008 keeps metadata derived, not authoritative).
pub fn decode_chunk(text: &str) -> Result<(ChunkId, ChunkData), TerrainAssetError> {
    let file: ChunkFile =
        ron::from_str(text).map_err(|err| TerrainAssetError::Ron(err.to_string()))?;
    if file.version != CHUNK_FORMAT_VERSION {
        return Err(TerrainAssetError::UnsupportedVersion {
            found: file.version,
            expected: CHUNK_FORMAT_VERSION,
        });
    }

    let heightfield =
        Heightfield::from_samples(file.samples_per_edge, file.spacing_meters, file.samples)
            .map_err(TerrainAssetError::Heightfield)?;

    let metadata = TerrainMetadata::from_heightfield(&heightfield);
    if metadata.height_min != file.height_min || metadata.height_max != file.height_max {
        return Err(TerrainAssetError::MetadataMismatch {
            x: file.x,
            z: file.z,
            stored_min: file.height_min,
            stored_max: file.height_max,
            computed_min: metadata.height_min,
            computed_max: metadata.height_max,
        });
    }

    let id = ChunkId::new(ChunkCoord::new(file.x, file.z));
    Ok((id, ChunkData::new(heightfield, Vec::new())))
}

/// Decode a height chunk file into a sync-load pipeline payload without
/// loading optional albedo sidecars (tests / terrain-import tooling only).
#[cfg(any(test, feature = "terrain-import"))]
pub fn decode_chunk_payload(text: &str) -> Result<(ChunkId, TerrainChunkPayload), TerrainAssetError> {
    let (id, chunk_data) = decode_chunk(text)?;
    Ok((id, TerrainChunkPayload::new(chunk_data, None)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::asset::{ManifestChunk, ManifestConfig};

    fn sample_chunk_file() -> ChunkFile {
        // 3x3 tile, spacing 128 -> 256 m chunk. Heights row*10 + col.
        let mut samples = Vec::new();
        for row in 0..3 {
            for col in 0..3 {
                samples.push((row * 10 + col) as f32);
            }
        }
        ChunkFile {
            version: CHUNK_FORMAT_VERSION,
            x: 1,
            z: 2,
            samples_per_edge: 3,
            spacing_meters: 128.0,
            samples,
            height_min: 0.0,
            height_max: 22.0,
        }
    }

    #[test]
    fn decodes_chunk_round_trip_through_ron() {
        let file = sample_chunk_file();
        let text = ron::to_string(&file).unwrap();
        let (id, data) = decode_chunk(&text).unwrap();

        assert_eq!(id, ChunkId::new(ChunkCoord::new(1, 2)));
        assert_eq!(data.heightfield.samples_per_edge(), 3);
        assert_eq!(data.metadata.height_min, 0.0);
        assert_eq!(data.metadata.height_max, 22.0);
    }

    #[test]
    fn rejects_metadata_mismatch() {
        let mut file = sample_chunk_file();
        file.height_max = 999.0;
        let text = ron::to_string(&file).unwrap();
        assert!(matches!(
            decode_chunk(&text),
            Err(TerrainAssetError::MetadataMismatch { .. })
        ));
    }

    #[test]
    fn rejects_unsupported_chunk_version() {
        let mut file = sample_chunk_file();
        file.version = 999;
        let text = ron::to_string(&file).unwrap();
        assert!(matches!(
            decode_chunk(&text),
            Err(TerrainAssetError::UnsupportedVersion { found: 999, .. })
        ));
    }

    #[test]
    fn decodes_manifest_round_trip_through_ron() {
        let manifest = Manifest {
            version: MANIFEST_FORMAT_VERSION,
            config: ManifestConfig {
                chunk_size_meters: 256.0,
                units_per_meter: 1.0,
                meters_per_sample: 1.0,
            },
            chunks: vec![ManifestChunk::at(1, 2, "chunks/1_2.ron")],
        };
        let text = ron::to_string(&manifest).unwrap();
        assert_eq!(decode_manifest(&text).unwrap(), manifest);
    }

    #[test]
    fn decodes_manifest_without_albedo_path_field() {
        let text = r#"
(
    version: 1,
    config: (
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
        meters_per_sample: 1.0,
    ),
    chunks: [
        (x: 0, z: 0, path: "chunks/0_0.ron"),
    ],
)
"#;
        let manifest = decode_manifest(text).unwrap();
        assert_eq!(manifest.chunks.len(), 1);
        assert_eq!(manifest.chunks[0].albedo_path, None);
    }

    #[test]
    fn decode_chunk_payload_carries_no_albedo_from_height_ron() {
        let file = sample_chunk_file();
        let text = ron::to_string(&file).unwrap();
        let (_, payload) = decode_chunk_payload(&text).unwrap();
        assert!(payload.albedo.is_none());
        assert_eq!(payload.chunk_data.heightfield.samples_per_edge(), 3);
    }
}
