use bevy::prelude::*;

/// Errors during biome mask PNG import (ADR-024).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiomeImportError {
    Io(String),
    PngDecode(String),
    UnsupportedColorType { color_type: String },
    UnsupportedBitDepth { bit_depth: String },
    EmptyImage,
    DimensionMismatch {
        expected_len: usize,
        actual_len: usize,
    },
}

impl std::fmt::Display for BiomeImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "biome mask io error: {msg}"),
            Self::PngDecode(msg) => write!(f, "biome mask png decode error: {msg}"),
            Self::UnsupportedColorType { color_type } => {
                write!(f, "unsupported biome mask color type: {color_type}")
            }
            Self::UnsupportedBitDepth { bit_depth } => {
                write!(f, "unsupported biome mask bit depth: {bit_depth}")
            }
            Self::EmptyImage => write!(f, "biome mask image is empty"),
            Self::DimensionMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "biome mask pixel buffer length mismatch: expected {expected_len}, got {actual_len}"
            ),
        }
    }
}

impl std::error::Error for BiomeImportError {}
