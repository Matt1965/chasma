//! Asset sizing import/dev report (ADR-097 DT1).

use serde::{Deserialize, Serialize};

use crate::world::authoring_transform::{AuthoringScale, QuantizedOrientation};

use super::definition::{SizeReferenceAxis, SourceBoundsOrigin, SourceDimensions};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetSizingReport {
    pub definition_id: String,
    pub definition_kind: String,
    pub asset_path: String,
    pub render_key: String,
    pub scene_selected: Option<String>,
    pub source_dimensions: Option<SourceDimensions>,
    pub source_bounds_origin: Option<SourceBoundsOrigin>,
    pub desired_width_meters: Option<f32>,
    pub desired_height_meters: Option<f32>,
    pub desired_depth_meters: Option<f32>,
    pub reference_axis: Option<SizeReferenceAxis>,
    pub exact_calculated_scale: Option<[f32; 3]>,
    pub quantized_baseline_scale: Option<AuthoringScale>,
    pub approximate_final_dimensions: Option<SourceDimensions>,
    pub rotation_correction: QuantizedOrientation,
    pub model_offset: [f32; 3],
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl AssetSizingReport {
    pub fn sort_key(&self) -> (&str, &str) {
        (&self.definition_kind, &self.definition_id)
    }
}

impl Default for AssetSizingReport {
    fn default() -> Self {
        Self {
            definition_id: String::new(),
            definition_kind: String::new(),
            asset_path: String::new(),
            render_key: String::new(),
            scene_selected: None,
            source_dimensions: None,
            source_bounds_origin: None,
            desired_width_meters: None,
            desired_height_meters: None,
            desired_depth_meters: None,
            reference_axis: None,
            exact_calculated_scale: None,
            quantized_baseline_scale: None,
            approximate_final_dimensions: None,
            rotation_correction: QuantizedOrientation::IDENTITY,
            model_offset: [0.0; 3],
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }
}

pub fn sort_reports(reports: &mut [AssetSizingReport]) {
    reports.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
}
