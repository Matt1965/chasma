//! Water material builders (ADR-053 E11 / WATER-DEPTH-1).

pub use super::ocean_material::{
    EnvironmentOceanMaterial, OceanDepthUniform, WaterDepthPresentation, build_ocean_depth_uniform,
    build_ocean_material, build_ocean_material as build_water_material,
    deep_ocean_color_from_shallow, evaluate_depth_response_factor,
};
