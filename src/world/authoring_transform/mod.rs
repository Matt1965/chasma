//! Authoritative transform value types for world authoring (ADR-097 DT1).
//!
//! Distinct from Bevy ECS presentation [`bevy::prelude::Transform`].

mod capabilities;
mod orientation;
mod scale;
mod transform;

pub use capabilities::{BuildingTransformSafetyClass, TransformCapabilities};
pub use orientation::{
    OrientationError, QuantizedOrientation, canonicalize_millidegrees, degrees_to_millidegrees,
};
pub use scale::{
    AuthoringScale, FixedScale, SCALE_MILLI_MAX, SCALE_MILLI_MIN, SCALE_MILLI_ONE, ScaleError,
};
pub use transform::AuthoringTransform;
