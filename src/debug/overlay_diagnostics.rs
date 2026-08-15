//! Overlay status lines for Navigation Editor diagnostics (IN-11eO).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::debug::path_trace::{PathTraceStatus, UnitPathDiagnosticStore};
use crate::debug::settings::DebugOverlaySettings;
use crate::dev::{BlueprintInspectionState, WorldInspectorState};
use crate::units::input::SelectedUnits;
use crate::world::{PortalType, WorldData};

/// Latest overlay layer status for dev UI and diagnostics.
#[derive(Resource, Debug, Clone, Default)]
pub struct NavigationOverlayDiagnostics {
    pub authored_blueprint: String,
    pub runtime_entrances: String,
    pub selected_unit_path: String,
    pub navigation_authority: String,
    pub authored_runtime_summary: String,
}

#[cfg(feature = "dev")]
pub fn sync_navigation_overlay_diagnostics(
    settings: Res<DebugOverlaySettings>,
    world: Res<WorldData>,
    world_selection: Res<WorldSelectionState>,
    selection: Res<SelectedUnits>,
    inspection: Res<BlueprintInspectionState>,
    inspector: Res<WorldInspectorState>,
    registry: Res<crate::dev::DevWindowRegistry>,
    path_store: Res<UnitPathDiagnosticStore>,
    mut diagnostics: ResMut<NavigationOverlayDiagnostics>,
) {
    let persisted_blueprint_id = inspector
        .blueprint_snapshot
        .as_ref()
        .and_then(|snap| snap.blueprint_id.clone());
    let persisted_regions = inspector
        .blueprint_snapshot
        .as_ref()
        .and_then(|snap| snap.resolved_blueprint.as_ref())
        .map(|bp| bp.floors.iter().map(|floor| floor.regions.len()).sum());
    let persisted_entrances = inspector
        .blueprint_snapshot
        .as_ref()
        .and_then(|snap| snap.resolved_blueprint.as_ref())
        .map(|bp| bp.entrances.len());

    diagnostics.authored_blueprint = authored_blueprint_status(
        settings.nav_blueprint,
        &world_selection,
        &inspection,
        persisted_blueprint_id.as_deref(),
        crate::debug::settings::navigation_editor_authored_session(&inspection, &registry),
    );
    diagnostics.runtime_entrances =
        runtime_entrances_status(settings.nav_entrances, &world, &world_selection);
    diagnostics.selected_unit_path = selected_unit_path_status(
        settings.path,
        &world,
        &selection,
        &world_selection,
        &path_store,
    );
    diagnostics.authored_runtime_summary = authored_runtime_comparison(
        &world,
        &world_selection,
        &inspection,
        persisted_regions,
        persisted_entrances,
    );
    diagnostics.navigation_authority = navigation_authority_status(
        settings.nav_blockers || settings.nav_blueprint,
        &world,
        &world_selection,
        &inspector,
    );
}

fn authored_blueprint_status(
    enabled: bool,
    world_selection: &WorldSelectionState,
    inspection: &BlueprintInspectionState,
    persisted_blueprint_id: Option<&str>,
    editor_session: bool,
) -> String {
    if editor_session {
        return "Authored Blueprint: Navigation Editor session (geometry visible)".into();
    }
    if !enabled {
        return String::new();
    }
    let building_id = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten();
    if building_id.is_none() {
        return "Authored Blueprint: Select a building.".into();
    }
    if inspection.editing || inspection.working_copy.is_some() {
        return "Authored Blueprint: Working Copy".into();
    }
    if let Some(id) = persisted_blueprint_id {
        return format!("Authored Blueprint: Persisted {id}");
    }
    "Authored Blueprint: No blueprint resolved".into()
}

fn runtime_entrances_status(
    enabled: bool,
    world: &WorldData,
    world_selection: &WorldSelectionState,
) -> String {
    if !enabled {
        return String::new();
    }
    let building_id = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten();
    if building_id.is_none() {
        return "Runtime Entrances: Select a building.".into();
    }
    let building_id = building_id.expect("checked");
    let portals: Vec<_> = world
        .space_registry()
        .portals()
        .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
        .filter(|(_, portal)| {
            portal.portal_type == PortalType::ExteriorEntrance
                || portal.portal_type == PortalType::Doorway
        })
        .collect();
    if portals.is_empty() {
        if world
            .building_navigation_runtime()
            .get(building_id)
            .is_none()
        {
            return "Runtime Entrances: Selected building is not runtime-activated.".into();
        }
        return "Runtime Entrances: 0 active portals for selected building".into();
    }
    format!(
        "Runtime Entrances: {} portal(s) active for selected building",
        portals.len()
    )
}

fn selected_unit_path_status(
    enabled: bool,
    world: &WorldData,
    selection: &SelectedUnits,
    world_selection: &WorldSelectionState,
    path_store: &UnitPathDiagnosticStore,
) -> String {
    if !enabled {
        return String::new();
    }
    let unit_id = world_selection
        .primary_unit(selection)
        .or_else(|| selection.iter().next());
    if unit_id.is_none() {
        return "Selected Unit Path: Select a unit.".into();
    }
    let unit_id = unit_id.expect("checked");
    if let Some(trace) = path_store.latest_for_unit(unit_id) {
        if trace.status == PathTraceStatus::Failed {
            let reason = trace
                .failure_reason
                .as_deref()
                .unwrap_or("movement blocked");
            return format!("Selected Unit Path: Last path failed: {reason}.");
        }
        return format!(
            "Selected Unit Path: {} waypoints · {:?}",
            trace.path.waypoints.len(),
            trace.status
        );
    }
    if let Some(record) = world.get_unit(unit_id) {
        if matches!(record.state, crate::world::UnitState::Moving { .. }) {
            return "Selected Unit Path: Active path (syncing).".into();
        }
    }
    "Selected Unit Path: Selected unit has no active or recorded path.".into()
}

fn navigation_authority_status(
    enabled: bool,
    world: &WorldData,
    world_selection: &WorldSelectionState,
    inspector: &WorldInspectorState,
) -> String {
    if !enabled {
        return String::new();
    }
    let building_id = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten();
    if building_id.is_none() {
        return "Navigation Authority: Select a building.".into();
    }
    let building_id = building_id.expect("checked");
    match crate::world::building_navigation_movement_authority(world, building_id) {
        crate::world::BuildingNavigationMovementAuthority::BlueprintControlled(_) => {
            let runtime = world.building_navigation_runtime().get(building_id);
            let blueprint_id = runtime
                .map(|r| r.blueprint_id.as_str())
                .unwrap_or("unknown");
            let region_count = runtime.map(|r| r.regions.len()).unwrap_or(0);
            let entrance_count = inspector
                .blueprint_snapshot
                .as_ref()
                .and_then(|snap| snap.resolved_blueprint.as_ref())
                .map(|bp| bp.entrances.len())
                .unwrap_or(0);
            let runtime_portals = world
                .space_registry()
                .portals()
                .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
                .filter(|(_, portal)| portal.portal_type == PortalType::ExteriorEntrance)
                .count();
            format!(
                "Navigation Authority: Blueprint · id={blueprint_id} · regions={region_count} \
                 entrances={entrance_count} · runtime regions={region_count} portals={runtime_portals}"
            )
        }
        crate::world::BuildingNavigationMovementAuthority::Ghost => {
            let definition = world
                .get_building(building_id)
                .map(|record| record.definition_id.as_str())
                .unwrap_or("unknown");
            format!(
                "Navigation Authority: Ghost (no hydrated runtime blueprint) · definition={definition}"
            )
        }
    }
}

fn authored_runtime_comparison(
    world: &WorldData,
    world_selection: &WorldSelectionState,
    inspection: &BlueprintInspectionState,
    persisted_regions: Option<usize>,
    persisted_entrances: Option<usize>,
) -> String {
    let building_id = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten();
    if building_id.is_none() {
        return String::new();
    }
    let building_id = building_id.expect("checked");
    let authored_regions = inspection
        .working_copy
        .as_ref()
        .map(|bp| bp.floors.iter().map(|floor| floor.regions.len()).sum())
        .or(persisted_regions)
        .unwrap_or(0);
    let authored_entrances = inspection
        .working_copy
        .as_ref()
        .map(|bp| bp.entrances.len())
        .or(persisted_entrances)
        .unwrap_or(0);
    let runtime_regions = world
        .building_navigation_runtime()
        .get(building_id)
        .map(|runtime| runtime.regions.len())
        .unwrap_or(0);
    let runtime_portals = world
        .space_registry()
        .portals()
        .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
        .filter(|(_, portal)| {
            portal.portal_type == PortalType::ExteriorEntrance
                || portal.portal_type == PortalType::Doorway
        })
        .count();
    let summary = if authored_regions == runtime_regions && authored_entrances == runtime_portals {
        "Match"
    } else if runtime_regions == 0 && authored_regions > 0 {
        "Activation missing"
    } else if runtime_portals == 0 && authored_entrances > 0 {
        "Activation missing"
    } else if authored_regions == 0 && runtime_regions > 0 {
        "Runtime without authored"
    } else {
        "Mismatch"
    };
    format!(
        "Authored: {authored_regions} region(s) / {authored_entrances} entrance(s) | Runtime: \
         {runtime_regions} region(s) / {runtime_portals} portal(s) | {summary}"
    )
}
