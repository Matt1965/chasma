//! Settlement placement constants (ADR-133).

use super::super::state::SettlementKind;

/// Extra clearance required between settlement boundaries at placement time.
pub const SETTLEMENT_PLACEMENT_MARGIN_METERS: f32 = 2.0;

/// Default initial boundary radius for [`SettlementKind::Town`] at creation.
pub const DEFAULT_TOWN_BOUNDARY_RADIUS_METERS: f32 = 64.0;

/// Initial boundary radius supplied by settlement kind at creation time only.
pub fn initial_boundary_radius_meters(kind: SettlementKind) -> f32 {
    match kind {
        SettlementKind::Town | SettlementKind::Village => DEFAULT_TOWN_BOUNDARY_RADIUS_METERS,
        SettlementKind::Outpost | SettlementKind::Camp => 48.0,
        SettlementKind::Hive | SettlementKind::Pack | SettlementKind::Herd => 32.0,
    }
}
