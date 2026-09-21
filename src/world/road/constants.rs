//! Centralized authoring tolerances for road junction connectivity.

/// World-space snap radius for endpoint and tee junction authoring.
pub const JUNCTION_SNAP_RADIUS_M: f32 = 2.0;

/// Sample spacing used when projecting points onto road splines and detecting crossings.
pub const ROAD_CONNECTIVITY_SAMPLE_SPACING_M: f32 = 2.0;

/// Spatial tolerance when collapsing duplicate derived crossing detections.
pub const CROSSING_DEDUP_RADIUS_M: f32 = 1.5;

/// Normalized arc-length tolerance when collapsing duplicate crossing parameters.
pub const CROSSING_PARAMETER_TOLERANCE: f32 = 0.02;

/// Ground crossings within this distance of a road endpoint are handled by endpoint snap.
pub const CROSSING_ENDPOINT_EXCLUSION_M: f32 = 2.0;
