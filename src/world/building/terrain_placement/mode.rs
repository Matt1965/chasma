//! Authored terrain placement behavior per building definition.

use bevy::prelude::Reflect;
use serde::{Deserialize, Serialize};

/// How a building's authoritative pose relates to terrain under its footprint.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Reflect,
)]
pub enum TerrainPlacementMode {
    /// Level floor at highest terrain under footprint; derived foundation skirt fills gaps.
    #[default]
    LevelFoundation,
    /// Rigid tilt to best-fit terrain plane with vertical clearance.
    ConformToTerrain,
}

impl TerrainPlacementMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::LevelFoundation => "LevelFoundation",
            Self::ConformToTerrain => "ConformToTerrain",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "levelfoundation" | "level_foundation" | "level" => Some(Self::LevelFoundation),
            "conformtoterrain" | "conform_to_terrain" | "conform" => Some(Self::ConformToTerrain),
            _ => None,
        }
    }
}

/// Centralized mode defaults (tuning values).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainPlacementDefaults {
    pub max_height_variation_meters: f32,
    pub max_foundation_depth_meters: f32,
    pub max_plane_fit_rms_meters: f32,
    pub max_plane_fit_peak_meters: f32,
}

impl TerrainPlacementDefaults {
    pub fn for_mode(mode: TerrainPlacementMode) -> Self {
        match mode {
            TerrainPlacementMode::LevelFoundation => Self {
                max_height_variation_meters: 2.0,
                max_foundation_depth_meters: 2.0,
                max_plane_fit_rms_meters: 0.0,
                max_plane_fit_peak_meters: 0.0,
            },
            TerrainPlacementMode::ConformToTerrain => Self {
                max_height_variation_meters: 3.0,
                max_foundation_depth_meters: 0.0,
                max_plane_fit_rms_meters: 0.35,
                max_plane_fit_peak_meters: 0.75,
            },
        }
    }
}

/// Clearance above the rendered terrain surface (presentation meters).
pub const PRESENTATION_TERRAIN_CLEARANCE_METERS: f32 = 0.05;

/// Convert presentation clearance into authoritative simulation meters for a terrain scale.
pub fn terrain_clearance_sim(terrain_vertical_scale: f32) -> f32 {
    PRESENTATION_TERRAIN_CLEARANCE_METERS / terrain_vertical_scale.max(1.0)
}

/// Minimum visible foundation depth before spawning skirt geometry (presentation meters).
pub const FOUNDATION_SKIRT_MIN_DEPTH: f32 = 0.08;

/// Maximum visible foundation depth allowed for level buildings (presentation meters).
pub const MAX_FOUNDATION_VISIBLE_DEPTH_METERS: f32 = 2.0;

/// Default outward embankment slope for derived foundation skirts.
pub const FOUNDATION_SLOPE_DEGREES: f32 = 45.0;

/// Horizontal run per meter of vertical drop at [`FOUNDATION_SLOPE_DEGREES`].
pub fn foundation_slope_run_per_meter_drop() -> f32 {
    FOUNDATION_SLOPE_DEGREES.to_radians().tan()
}

/// Small render-space sink so foundation skirts close against exaggerated terrain.
pub const FOUNDATION_TERRAIN_PENETRATION_FUDGE_METERS: f32 = 0.03;

/// World-space UV tiling for derived foundation stone (meters per texture repeat).
pub const FOUNDATION_TEXTURE_TILE_METERS: f32 = 1.5;
