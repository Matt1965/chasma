//! Settlement anchor world objects (ADR-133).

mod constants;
mod error;
mod id;
mod overlap;
mod record;
mod store;

pub use constants::{
    DEFAULT_TOWN_BOUNDARY_RADIUS_METERS, SETTLEMENT_PLACEMENT_MARGIN_METERS,
    initial_boundary_radius_meters,
};
pub use error::SettlementCreationError;
pub use id::SettlementAnchorId;
pub use overlap::{required_center_separation_meters, settlement_overlaps_existing};
pub use record::SettlementAnchorRecord;
pub use store::SettlementAnchorStore;
