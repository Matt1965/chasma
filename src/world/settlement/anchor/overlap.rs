//! Settlement boundary overlap validation (ADR-133).

use crate::world::{ChunkLayout, WorldPosition, xz_distance};

use super::super::record::SettlementRecord;
use super::constants::SETTLEMENT_PLACEMENT_MARGIN_METERS;

/// Minimum center separation required so two settlement boundaries do not overlap.
pub fn required_center_separation_meters(radius_a: f32, radius_b: f32) -> f32 {
    radius_a + radius_b + SETTLEMENT_PLACEMENT_MARGIN_METERS
}

/// Whether placing a settlement at `center` with `radius` would overlap `existing`.
pub fn settlement_overlaps_existing(
    center: WorldPosition,
    radius: f32,
    existing: &SettlementRecord,
    layout: ChunkLayout,
) -> bool {
    let distance = xz_distance(center, existing.center, layout);
    distance < required_center_separation_meters(radius, existing.boundary_radius_meters)
}
