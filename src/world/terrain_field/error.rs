//! Terrain field errors (ADR-101 TF1).

use super::id::TerrainFieldId;
use crate::world::{ChunkCoord, ChunkId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerrainFieldDefinitionError {
    DuplicateTerrainFieldId(TerrainFieldId),
    InvalidCategory(String),
    InvalidValueSemantics(String),
    InvalidOverlayOpacity,
    InvalidVisibilityCutoff,
    UnsortedQualitativeThresholds,
    QualitativeLabelCountMismatch,
    InvalidTerrainFieldId(String),
}

impl std::fmt::Display for TerrainFieldDefinitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateTerrainFieldId(id) => write!(f, "duplicate terrain field id `{id}`"),
            Self::InvalidCategory(value) => write!(f, "invalid terrain field category `{value}`"),
            Self::InvalidValueSemantics(value) => {
                write!(f, "invalid terrain field value semantics `{value}`")
            }
            Self::InvalidOverlayOpacity => {
                write!(f, "overlay opacity must be finite and in [0, 1]")
            }
            Self::InvalidVisibilityCutoff => write!(f, "invalid visibility cutoff"),
            Self::UnsortedQualitativeThresholds => {
                write!(f, "qualitative thresholds must be strictly increasing")
            }
            Self::QualitativeLabelCountMismatch => write!(
                f,
                "qualitative label count must match threshold count when labels are provided"
            ),
            Self::InvalidTerrainFieldId(value) => {
                write!(f, "invalid terrain field id `{value}`")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerrainFieldStorageError {
    DuplicateLayer(TerrainFieldId),
    LayerMissing(TerrainFieldId),
    TileMissing {
        field_id: TerrainFieldId,
        chunk: ChunkId,
    },
    DuplicateTile {
        field_id: TerrainFieldId,
        chunk: ChunkId,
    },
    InvalidSamplesPerEdge {
        found: u16,
        expected: u16,
    },
    InvalidSampleSpacing {
        found: f32,
        expected: f32,
    },
    InvalidTileSampleCount {
        found: usize,
        expected: usize,
    },
    TileChunkMismatch {
        tile_chunk: ChunkCoord,
        key_chunk: ChunkCoord,
    },
    SharedEdgeMismatch {
        field_id: TerrainFieldId,
        axis: SharedEdgeAxis,
        chunk_a: ChunkCoord,
        chunk_b: ChunkCoord,
        index: u16,
        value_a: u16,
        value_b: u16,
    },
    WorldConfigMismatch(String),
    CorruptTile {
        field_id: TerrainFieldId,
        chunk: ChunkId,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedEdgeAxis {
    EastWest,
    NorthSouth,
}

impl std::fmt::Display for TerrainFieldStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateLayer(id) => write!(f, "duplicate terrain field layer for `{id}`"),
            Self::LayerMissing(id) => write!(f, "terrain field layer missing for `{id}`"),
            Self::TileMissing { field_id, chunk } => {
                write!(
                    f,
                    "terrain field tile missing for `{field_id}` at chunk {:?}",
                    chunk.coord()
                )
            }
            Self::DuplicateTile { field_id, chunk } => write!(
                f,
                "duplicate terrain field tile for `{field_id}` at chunk {:?}",
                chunk.coord()
            ),
            Self::InvalidSamplesPerEdge { found, expected } => {
                write!(f, "invalid samples per edge {found}, expected {expected}")
            }
            Self::InvalidSampleSpacing { found, expected } => {
                write!(f, "invalid sample spacing {found} m, expected {expected} m")
            }
            Self::InvalidTileSampleCount { found, expected } => {
                write!(f, "invalid tile sample count {found}, expected {expected}")
            }
            Self::TileChunkMismatch {
                tile_chunk,
                key_chunk,
            } => write!(
                f,
                "tile chunk ({}, {}) does not match key ({}, {})",
                tile_chunk.x, tile_chunk.z, key_chunk.x, key_chunk.z
            ),
            Self::SharedEdgeMismatch {
                field_id,
                axis,
                chunk_a,
                chunk_b,
                index,
                value_a,
                value_b,
            } => write!(
                f,
                "shared edge mismatch on {axis:?} for `{field_id}` between ({}, {}) and ({}, {}) at index {index}: {value_a} vs {value_b}",
                chunk_a.x, chunk_a.z, chunk_b.x, chunk_b.z
            ),
            Self::WorldConfigMismatch(message) => write!(f, "world config mismatch: {message}"),
            Self::CorruptTile {
                field_id,
                chunk,
                reason,
            } => write!(
                f,
                "corrupt terrain field tile for `{field_id}` at {:?}: {reason}",
                chunk.coord()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerrainFieldQueryError {
    FieldDefinitionMissing(TerrainFieldId),
    FieldDisabled(TerrainFieldId),
    FieldDataUnavailable(FieldAvailabilityReason),
    OutsideAuthoredWorld,
    InvalidWorldCoordinate,
    FixedPointInterpolationOverflow,
    AreaRegionEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldAvailabilityReason {
    LayerMissing,
    TileMissing,
    TileNotResident,
    CorruptTile,
    InvalidCoordinate,
}

impl From<super::sample::FieldAvailability> for FieldAvailabilityReason {
    fn from(value: super::sample::FieldAvailability) -> Self {
        use super::sample::FieldAvailability;
        match value {
            FieldAvailability::Available => FieldAvailabilityReason::TileMissing,
            FieldAvailability::FieldLayerMissing => FieldAvailabilityReason::LayerMissing,
            FieldAvailability::TileMissing => FieldAvailabilityReason::TileMissing,
            FieldAvailability::TileNotResident => FieldAvailabilityReason::TileNotResident,
            FieldAvailability::CorruptTile => FieldAvailabilityReason::CorruptTile,
            FieldAvailability::InvalidCoordinate => FieldAvailabilityReason::InvalidCoordinate,
            FieldAvailability::FieldDefinitionMissing
            | FieldAvailability::FieldDisabled
            | FieldAvailability::OutsideWorld => FieldAvailabilityReason::TileMissing,
        }
    }
}

impl std::fmt::Display for TerrainFieldQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FieldDefinitionMissing(id) => {
                write!(f, "terrain field definition missing for `{id}`")
            }
            Self::FieldDisabled(id) => write!(f, "terrain field `{id}` is disabled"),
            Self::FieldDataUnavailable(reason) => {
                write!(f, "terrain field data unavailable: {reason:?}")
            }
            Self::OutsideAuthoredWorld => write!(f, "position outside authored world"),
            Self::InvalidWorldCoordinate => write!(f, "invalid world coordinate for field query"),
            Self::FixedPointInterpolationOverflow => {
                write!(f, "fixed-point interpolation overflow")
            }
            Self::AreaRegionEmpty => write!(f, "area sample region is empty"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerrainFieldLoadError {
    ManifestMissing(String),
    ManifestVersionUnsupported {
        found: u32,
        expected: u32,
    },
    ManifestParse(String),
    TileParse {
        path: String,
        message: String,
    },
    TileAssetMissing {
        path: String,
    },
    SourceVersionMismatch {
        field_id: TerrainFieldId,
        manifest: String,
        tile: String,
    },
    WorldConfigMismatch(String),
    Storage(TerrainFieldStorageError),
    Catalog(TerrainFieldCatalogError),
}

impl From<TerrainFieldStorageError> for TerrainFieldLoadError {
    fn from(value: TerrainFieldStorageError) -> Self {
        Self::Storage(value)
    }
}

impl std::fmt::Display for TerrainFieldLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManifestMissing(path) => write!(f, "terrain field manifest missing: {path}"),
            Self::ManifestVersionUnsupported { found, expected } => write!(
                f,
                "unsupported terrain field manifest version {found}, expected {expected}"
            ),
            Self::ManifestParse(message) => {
                write!(f, "terrain field manifest parse error: {message}")
            }
            Self::TileParse { path, message } => {
                write!(f, "terrain field tile parse error at {path}: {message}")
            }
            Self::TileAssetMissing { path } => {
                write!(f, "terrain field tile asset missing: {path}")
            }
            Self::SourceVersionMismatch {
                field_id,
                manifest,
                tile,
            } => write!(
                f,
                "source version mismatch for `{field_id}`: manifest={manifest}, tile={tile}"
            ),
            Self::WorldConfigMismatch(message) => write!(f, "world config mismatch: {message}"),
            Self::Storage(err) => write!(f, "{err}"),
            Self::Catalog(err) => write!(f, "{err}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerrainFieldCatalogError {
    DuplicateId(TerrainFieldId),
    InvalidDefinition(TerrainFieldDefinitionError),
    RonParse(String),
    RonIo(String),
}

impl std::fmt::Display for TerrainFieldCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate terrain field id `{id}`"),
            Self::InvalidDefinition(err) => write!(f, "{err}"),
            Self::RonParse(message) => {
                write!(f, "terrain field catalog RON parse error: {message}")
            }
            Self::RonIo(message) => write!(f, "terrain field catalog IO error: {message}"),
        }
    }
}
