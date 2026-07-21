//! Metric asset sizing definition embedded in content catalogs (ADR-097 DT1).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::authoring_transform::{AuthoringScale, QuantizedOrientation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect, Default)]
pub enum SizeReferenceAxis {
    Width,
    #[default]
    Height,
    Depth,
}

impl SizeReferenceAxis {
    pub fn label(self) -> &'static str {
        match self {
            Self::Width => "Width",
            Self::Height => "Height",
            Self::Depth => "Depth",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "width" | "w" | "x" => Some(Self::Width),
            "height" | "h" | "y" => Some(Self::Height),
            "depth" | "d" | "z" => Some(Self::Depth),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum SourceBoundsOrigin {
    ExplicitCatalog,
    NamedNode,
    CombinedVisibleMeshes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect, Default)]
pub enum SizingMigrationState {
    #[default]
    MissingSizingData,
    LegacyExplicitScale,
    MetricConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Reflect)]
pub struct SourceDimensions {
    pub width_meters: f32,
    pub height_meters: f32,
    pub depth_meters: f32,
}

impl SourceDimensions {
    pub fn axis(self, axis: SizeReferenceAxis) -> f32 {
        match axis {
            SizeReferenceAxis::Width => self.width_meters,
            SizeReferenceAxis::Height => self.height_meters,
            SizeReferenceAxis::Depth => self.depth_meters,
        }
    }

    pub fn is_valid(self) -> bool {
        self.width_meters.is_finite()
            && self.height_meters.is_finite()
            && self.depth_meters.is_finite()
            && self.width_meters > 0.0
            && self.height_meters > 0.0
            && self.depth_meters > 0.0
    }
}

/// Shared metric sizing fields embedded by Unit, Doodad, and Building definitions.
///
/// AT1 (ADR-126/127): this struct is the **authoritative** catalog home for:
/// - desired metric dimensions
/// - measured / explicit source dimensions
/// - baked baseline import scale
/// - pivot correction (`model_local_offset_meters`)
/// - import rotation correction
///
/// Building legacy fields (`model_local_offset`, `model_yaw_correction_degrees`) are mirrors only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
pub struct AssetSizingDefinition {
    pub desired_width_meters: Option<f32>,
    pub desired_height_meters: Option<f32>,
    pub desired_depth_meters: Option<f32>,
    pub size_reference_axis: Option<SizeReferenceAxis>,
    pub source_bounds_node: Option<String>,
    pub explicit_source_dimensions: Option<SourceDimensions>,
    pub model_local_offset_meters: Vec3,
    pub rotation_correction: QuantizedOrientation,
    pub explicit_baseline_scale: Option<AuthoringScale>,
    pub calculated_source_bounds: Option<SourceDimensions>,
    pub calculated_baseline_scale: Option<AuthoringScale>,
    pub source_bounds_origin: Option<SourceBoundsOrigin>,
    /// Offline import may normalize mm/cm GLB bounds before baseline quantization.
    /// Runtime presentation divides the baked baseline by this factor so the raw
    /// mesh vertices (still in export units) reach desired meters.
    #[serde(default = "default_source_bounds_unit_divisor")]
    pub source_bounds_unit_divisor: f32,
    pub migration_state: SizingMigrationState,
}

fn default_source_bounds_unit_divisor() -> f32 {
    1.0
}

impl Default for AssetSizingDefinition {
    fn default() -> Self {
        Self {
            desired_width_meters: None,
            desired_height_meters: None,
            desired_depth_meters: None,
            size_reference_axis: None,
            source_bounds_node: None,
            explicit_source_dimensions: None,
            model_local_offset_meters: Vec3::ZERO,
            rotation_correction: QuantizedOrientation::IDENTITY,
            explicit_baseline_scale: None,
            calculated_source_bounds: None,
            calculated_baseline_scale: None,
            source_bounds_origin: None,
            source_bounds_unit_divisor: default_source_bounds_unit_divisor(),
            migration_state: SizingMigrationState::MissingSizingData,
        }
    }
}

impl AssetSizingDefinition {
    pub fn has_desired_dimensions(&self) -> bool {
        self.desired_width_meters.is_some()
            || self.desired_height_meters.is_some()
            || self.desired_depth_meters.is_some()
    }

    pub fn has_explicit_baseline(&self) -> bool {
        self.explicit_baseline_scale.is_some()
    }

    pub fn resolved_baseline_scale(&self) -> AuthoringScale {
        self.calculated_baseline_scale
            .or(self.explicit_baseline_scale)
            .unwrap_or(AuthoringScale::uniform_one())
    }

    pub fn resolved_source_bounds(&self) -> Option<SourceDimensions> {
        self.calculated_source_bounds
            .or(self.explicit_source_dimensions)
    }
}

/// Doodad collision seam fields (DT2); parsed in DT1 without changing occupancy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect, Default)]
pub enum DoodadCollisionShape {
    #[default]
    None,
    Circle,
    Ellipse,
    Rectangle,
    Baked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect, Default)]
pub enum DoodadGroundingMode {
    #[default]
    TerrainGrounded,
    Free,
    SupportSurface,
    FixedElevation,
}

impl DoodadGroundingMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "terraingrounded" | "terrain" | "grounded" => Some(Self::TerrainGrounded),
            "free" => Some(Self::Free),
            "supportsurface" | "support" => Some(Self::SupportSurface),
            "fixedelevation" | "fixed" => Some(Self::FixedElevation),
            _ => None,
        }
    }
}

impl DoodadCollisionShape {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" | "" => Some(Self::None),
            "circle" => Some(Self::Circle),
            "ellipse" => Some(Self::Ellipse),
            "rectangle" | "rect" => Some(Self::Rectangle),
            "baked" | "mask" => Some(Self::Baked),
            _ => None,
        }
    }
}
