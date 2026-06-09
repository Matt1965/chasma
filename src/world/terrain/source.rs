use std::path::PathBuf;

/// Describes where authoritative terrain data is imported from (ADR-009).
///
/// This is an import *descriptor* only: it names the source files. It performs
/// no decoding or loading; the importer (a later Phase 1 pass) consumes it. It
/// is deliberately not a render or asset type.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainSource {
    /// Path to the authoritative floating-point heightfield (EXR, ADR-003).
    pub heightfield_path: PathBuf,
    /// Optional mask layers to import alongside the heightfield.
    pub masks: Vec<MaskSource>,
}

/// A single mask layer to import, paired with the layer identifier it produces.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskSource {
    pub layer: String,
    pub path: PathBuf,
}

impl TerrainSource {
    pub fn new(heightfield_path: impl Into<PathBuf>) -> Self {
        Self {
            heightfield_path: heightfield_path.into(),
            masks: Vec::new(),
        }
    }
}
