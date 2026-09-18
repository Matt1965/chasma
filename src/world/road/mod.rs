//! Authored road network authority (development-authored static world data).
//!
//! # Authority
//!
//! [`RoadNetwork`] is the durable source of truth for roads. Control points store
//! authoritative simulation XZ only; road-bed elevation is derived later during
//! terrain baking from base terrain, not persisted on the spline.
//!
//! # Derived outputs (later phases)
//!
//! The following are intentionally **not** stored as authority and will be derived
//! from [`RoadNetwork`] in later work:
//!
//! - dense road polyline / arc-length samples
//! - per-chunk terrain deformation (effective height deltas)
//! - per-chunk terrain darkening masks
//! - persisted + derived junction connectivity graphs
//! - future strategic navigation / travel-cost graphs
//!
//! Do not introduce parallel authoritative representations for visual roads,
//! terrain roads, or navigation roads.

mod attachment;
mod control_point;
mod crossing;
mod error;
mod id;
mod junction;
mod load;
mod network;
mod road;
mod spline;
mod style;
mod validate;

pub use attachment::{RoadEndpointAttachment, RoadTeeAttachment};
pub use control_point::{CornerMode, RoadControlPoint};
pub use crossing::{RoadCrossingKind, RoadCrossingOverride};
pub use error::{RoadError, RoadLoadError};
pub use id::{JunctionId, RoadId};
pub use junction::{Junction, JunctionMember, JunctionMemberRole};
pub use load::{
    DEFAULT_WORLD_PACKAGE_DIR, load_road_network, load_road_network_from_path,
    load_road_network_from_world_package, parse_road_network_ron, road_network_ron_path,
    serialize_road_network_ron,
};
pub use network::{RoadNetwork, RoadNetworkRon, ROAD_NETWORK_SCHEMA_VERSION};
pub use road::Road;
pub use spline::{RoadSplineSample, sample_road_polyline, sample_road_spline_at_distance};
pub use style::{RoadStyleDefaults, RoadStyleId, RoadStyleOverrides};
pub use validate::validate_road_network;

#[cfg(test)]
mod tests;
