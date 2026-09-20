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
mod constants;
mod connectivity;
mod control_point;
mod crossing;
mod crossing_detect;
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
pub use connectivity::{
    SnapCandidate, SnapCandidateKind, apply_snap_candidate, detach_junction,
    endpoint_has_attachment, find_snap_candidate, finalize_endpoint_drag,
    generate_junction_id, is_road_endpoint_index, junction_id_for_endpoint,
    junction_world_position, move_connected_endpoint, move_endpoint_junction_group,
    refresh_all_tee_branches, refresh_tee_branches_for_host,
    remove_road_and_cleanup_junctions, try_snap_endpoint,
};
pub use constants::JUNCTION_SNAP_RADIUS_M;
pub use control_point::{CornerMode, RoadControlPoint};
pub use crossing::{RoadCrossingKind, RoadCrossingOverride};
pub use crossing_detect::{DerivedCrossing, derive_ground_crossings};
pub use error::{RoadError, RoadLoadError};
pub use id::{JunctionId, RoadId};
pub use junction::{Junction, JunctionMember, JunctionMemberRole};
pub use load::{
    DEFAULT_WORLD_PACKAGE_DIR, load_road_network, load_road_network_from_path,
    load_road_network_from_world_package, parse_road_network_ron, road_network_ron_path,
    save_road_network, save_road_network_to_path, serialize_road_network_ron,
};
pub use network::{RoadNetwork, RoadNetworkRon, ROAD_NETWORK_SCHEMA_VERSION};
pub use road::Road;
pub use spline::{
    RoadSplineProjection, RoadSplineSample, project_point_onto_road_spline, road_spline_length,
    sample_road_polyline, sample_road_spline_at_distance, sample_road_spline_at_t,
};
pub use style::{RoadStyleDefaults, RoadStyleId, RoadStyleOverrides};
pub use validate::validate_road_network;

#[cfg(test)]
mod tests;
