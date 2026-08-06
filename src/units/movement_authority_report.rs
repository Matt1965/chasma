//! Dev classification of movement authority traces (IN-11eR).

use bevy::prelude::*;

use crate::units::input::SelectedUnits;
use crate::world::{UnitId, WorldData};

/// Latest movement authority diagnostic for dev panels.
#[derive(Resource, Debug, Clone, Default)]
pub struct LatestMovementAuthorityReport {
    pub unit_id: Option<UnitId>,
    pub line: String,
}

/// Log movement authority diagnostics when a blocked frame is recorded for the selection.
pub fn report_movement_authority_for_selection(
    world: Res<WorldData>,
    selected: Res<SelectedUnits>,
    mut report: ResMut<LatestMovementAuthorityReport>,
) {
    let unit_id = selected.iter().next();
    let Some(unit_id) = unit_id else {
        report.unit_id = None;
        report.line.clear();
        return;
    };

    let trace = world.movement_authority_trace();
    let line = trace.diagnostic_line_for_unit(unit_id);
    if report.unit_id == Some(unit_id) && report.line == line {
        return;
    }
    report.unit_id = Some(unit_id);
    report.line = line.clone();

    if line.contains("blocked") || line.contains("violation") {
        warn!("movement authority U-{:04}: {}", unit_id.raw(), line);
    }
}
