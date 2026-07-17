//! Asset sizing errors (ADR-097 DT1).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetSizingError {
    AssetNotFound { path: String },
    SceneSelectionMissing,
    SourceBoundsNodeMissing { node: String },
    SourceBoundsUnavailable,
    SourceBoundsInvalid { message: String },
    SourceAxisZero { axis: String },
    DesiredDimensionsInvalid { message: String },
    ContradictorySizingInputs { message: String },
    InvalidReferenceAxis,
    BaselineScaleOutOfRange,
    QuantizationOverflow,
    NonUniformScaleUnsupported,
    SuspectedUnitMismatch { message: String },
    InvalidOrientationCorrection,
    InvalidModelOffset,
    BuildingVisualTopologyScaleMismatch { message: String },
    AmbiguousPartialDimensions,
}

impl fmt::Display for AssetSizingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AssetNotFound { path } => write!(f, "asset not found: {path}"),
            Self::SceneSelectionMissing => {
                write!(f, "GLB has multiple scenes; select one explicitly")
            }
            Self::SourceBoundsNodeMissing { node } => {
                write!(f, "source bounds node `{node}` not found")
            }
            Self::SourceBoundsUnavailable => write!(f, "source bounds unavailable"),
            Self::SourceBoundsInvalid { message } => write!(f, "invalid source bounds: {message}"),
            Self::SourceAxisZero { axis } => write!(f, "source {axis} extent is zero"),
            Self::DesiredDimensionsInvalid { message } => {
                write!(f, "invalid desired dimensions: {message}")
            }
            Self::ContradictorySizingInputs { message } => {
                write!(f, "contradictory sizing inputs: {message}")
            }
            Self::InvalidReferenceAxis => write!(f, "invalid size reference axis"),
            Self::BaselineScaleOutOfRange => write!(f, "baseline scale out of allowed range"),
            Self::QuantizationOverflow => write!(f, "scale quantization overflow"),
            Self::NonUniformScaleUnsupported => {
                write!(f, "non-uniform scale unsupported for this type")
            }
            Self::SuspectedUnitMismatch { message } => {
                write!(f, "suspected source unit mismatch: {message}")
            }
            Self::InvalidOrientationCorrection => write!(f, "invalid orientation correction"),
            Self::InvalidModelOffset => write!(f, "invalid model offset"),
            Self::BuildingVisualTopologyScaleMismatch { message } => {
                write!(f, "building visual scale would desync topology: {message}")
            }
            Self::AmbiguousPartialDimensions => {
                write!(
                    f,
                    "ambiguous partial desired dimensions without reference axis"
                )
            }
        }
    }
}

impl std::error::Error for AssetSizingError {}
