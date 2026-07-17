//! Imported mask source configuration (ADR-102).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::source_error::TerrainFieldSourceError;
use super::bounds::TerrainFieldWorldBounds;
use super::remap::TerrainFieldValueRemap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerrainFieldImageChannel {
    Luminance,
    Red,
    Green,
    Blue,
    Alpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerrainFieldImageOrientation {
    /// Image row 0 is minimum world Z (south), matching ADR-024 biome convention.
    RowZeroIsMinimumZ,
    /// Image row 0 is maximum world Z (north); rows are flipped when sampling.
    RowZeroIsMaximumZ,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerrainFieldResampling {
    Nearest,
    Bilinear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerrainFieldOutsideCoveragePolicy {
    Reject,
    ClampToEdge,
    FillZero,
}

/// Configuration for an imported grayscale/color mask (ADR-102).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedTerrainFieldSource {
    pub asset_path: String,
    pub channel: TerrainFieldImageChannel,
    pub orientation: TerrainFieldImageOrientation,
    pub world_bounds: TerrainFieldWorldBounds,
    pub resampling: TerrainFieldResampling,
    pub remap: TerrainFieldValueRemap,
    pub outside_coverage_policy: TerrainFieldOutsideCoveragePolicy,
}

impl Default for ImportedTerrainFieldSource {
    fn default() -> Self {
        Self {
            asset_path: String::new(),
            channel: TerrainFieldImageChannel::Luminance,
            orientation: TerrainFieldImageOrientation::RowZeroIsMinimumZ,
            world_bounds: TerrainFieldWorldBounds::new(0.0, 0.0, 1.0, 1.0),
            resampling: TerrainFieldResampling::Bilinear,
            remap: TerrainFieldValueRemap::full_range(),
            outside_coverage_policy: TerrainFieldOutsideCoveragePolicy::ClampToEdge,
        }
    }
}

impl ImportedTerrainFieldSource {
    pub fn validate(&self) -> Result<(), TerrainFieldSourceError> {
        if self.asset_path.trim().is_empty() {
            return Err(TerrainFieldSourceError::SourceImageMissing(
                "asset_path is empty".to_string(),
            ));
        }
        if self.world_bounds.extent_x <= 0.0 || self.world_bounds.extent_z <= 0.0 {
            return Err(TerrainFieldSourceError::InvalidWorldBounds(
                "extent must be positive".to_string(),
            ));
        }
        self.remap.validate()?;
        Ok(())
    }
}
