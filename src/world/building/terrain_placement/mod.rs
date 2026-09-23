mod foundation;
mod mode;
mod resolve;
mod sample;

#[cfg(test)]
mod tests;

pub use foundation::{
    FoundationPerimeterVertex, FoundationSkirtSpec, footprint_horizontal_span_meters,
    presentation_foundation_depth_meters,
};
pub use mode::{
    FOUNDATION_SLOPE_DEGREES, FOUNDATION_TEXTURE_TILE_METERS,
    FOUNDATION_TERRAIN_PENETRATION_FUDGE_METERS, MAX_FOUNDATION_VISIBLE_DEPTH_METERS,
    PRESENTATION_TERRAIN_CLEARANCE_METERS, TerrainPlacementMode, foundation_slope_run_per_meter_drop, terrain_clearance_sim,
};
pub use resolve::{
    ResolvedBuildingPlacement, derive_foundation_skirt_for_placement, resolve_building_placement,
    rotation_from_yaw_and_normal,
};
pub use sample::{
    presentation_plane_normal,
    sample_terrain_under_footprint, slope_degrees_from_plane_coefficients,
};
