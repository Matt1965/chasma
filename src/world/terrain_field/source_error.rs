//! Terrain field source/build errors (ADR-102).

use super::id::{TerrainFieldId, TerrainFieldSourceProfileId};

#[derive(Debug, Clone, PartialEq)]
pub enum TerrainFieldSourceError {
    TerrainFieldSourceProfileMissing(TerrainFieldSourceProfileId),
    DuplicateTerrainFieldSourceProfile(TerrainFieldSourceProfileId),
    UnsupportedSourceKind(String),
    InvalidSourceConfiguration(String),
    InvalidWorldBounds(String),
    InvalidSourceChannel(String),
    UnsupportedImageFormat(String),
    UnsupportedImageBitDepth(String),
    SourceImageMissing(String),
    SourceImageDecodeFailed(String),
    SourceImageEmpty,
    SourceImageDimensionMismatch {
        expected: (u32, u32),
        found: (u32, u32),
    },
    SourceImageAspectMismatch {
        image_aspect: f32,
        world_aspect: f32,
    },
    SourceImageOrientationInvalid(String),
    SourceImageChannelUnavailable(String),
    SourceValueRemapInvalid(String),
    GeneratorUnknown(String),
    GeneratorDependencyMissing(String),
    GeneratorDependencyCycle(String),
    GeneratorParameterInvalid(String),
    GeneratorVersionUnsupported {
        found: u32,
        expected: u32,
    },
    GenerationCoordinateInvalid,
    GenerationOverflow,
    TargetWorldConfigMismatch(String),
    TilePartitionFailed(String),
    SharedEdgeMismatch(String),
    OutputDirectoryUnavailable(String),
    TemporaryPackageWriteFailed(String),
    PackageCommitFailed(String),
    StaleTileCleanupFailed(String),
    SourceVersionHashFailed(String),
    FieldDefinitionMissing(TerrainFieldId),
}

impl std::fmt::Display for TerrainFieldSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TerrainFieldSourceProfileMissing(id) => {
                write!(f, "terrain field source profile missing: `{id}`")
            }
            Self::DuplicateTerrainFieldSourceProfile(id) => {
                write!(f, "duplicate terrain field source profile: `{id}`")
            }
            Self::UnsupportedSourceKind(kind) => write!(f, "unsupported source kind: {kind}"),
            Self::InvalidSourceConfiguration(msg) => {
                write!(f, "invalid source configuration: {msg}")
            }
            Self::InvalidWorldBounds(msg) => write!(f, "invalid world bounds: {msg}"),
            Self::InvalidSourceChannel(msg) => write!(f, "invalid source channel: {msg}"),
            Self::UnsupportedImageFormat(msg) => write!(f, "unsupported image format: {msg}"),
            Self::UnsupportedImageBitDepth(msg) => write!(f, "unsupported image bit depth: {msg}"),
            Self::SourceImageMissing(path) => write!(f, "source image missing: {path}"),
            Self::SourceImageDecodeFailed(msg) => write!(f, "source image decode failed: {msg}"),
            Self::SourceImageEmpty => write!(f, "source image is empty"),
            Self::SourceImageDimensionMismatch { expected, found } => write!(
                f,
                "source image dimension mismatch: expected {expected:?}, found {found:?}"
            ),
            Self::SourceImageAspectMismatch {
                image_aspect,
                world_aspect,
            } => write!(
                f,
                "source image aspect {image_aspect} != world aspect {world_aspect}"
            ),
            Self::SourceImageOrientationInvalid(msg) => {
                write!(f, "invalid image orientation: {msg}")
            }
            Self::SourceImageChannelUnavailable(msg) => {
                write!(f, "source image channel unavailable: {msg}")
            }
            Self::SourceValueRemapInvalid(msg) => write!(f, "invalid value remap: {msg}"),
            Self::GeneratorUnknown(kind) => write!(f, "unknown generator kind: {kind}"),
            Self::GeneratorDependencyMissing(dep) => {
                write!(f, "generator dependency missing: {dep}")
            }
            Self::GeneratorDependencyCycle(msg) => write!(f, "generator dependency cycle: {msg}"),
            Self::GeneratorParameterInvalid(msg) => write!(f, "invalid generator parameter: {msg}"),
            Self::GeneratorVersionUnsupported { found, expected } => write!(
                f,
                "unsupported generator version {found}, expected {expected}"
            ),
            Self::GenerationCoordinateInvalid => write!(f, "invalid generation coordinate"),
            Self::GenerationOverflow => write!(f, "generation arithmetic overflow"),
            Self::TargetWorldConfigMismatch(msg) => write!(f, "world config mismatch: {msg}"),
            Self::TilePartitionFailed(msg) => write!(f, "tile partition failed: {msg}"),
            Self::SharedEdgeMismatch(msg) => write!(f, "shared edge mismatch: {msg}"),
            Self::OutputDirectoryUnavailable(msg) => {
                write!(f, "output directory unavailable: {msg}")
            }
            Self::TemporaryPackageWriteFailed(msg) => {
                write!(f, "temporary package write failed: {msg}")
            }
            Self::PackageCommitFailed(msg) => write!(f, "package commit failed: {msg}"),
            Self::StaleTileCleanupFailed(msg) => write!(f, "stale tile cleanup failed: {msg}"),
            Self::SourceVersionHashFailed(msg) => write!(f, "source version hash failed: {msg}"),
            Self::FieldDefinitionMissing(id) => {
                write!(f, "terrain field definition missing: `{id}`")
            }
        }
    }
}

pub type TerrainFieldBuildResult<T> = Result<T, TerrainFieldSourceError>;
