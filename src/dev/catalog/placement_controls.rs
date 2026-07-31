//! Contextual placement control visibility (Slice 4).

use super::super::dev_mode::{DefinitionId, DevModeState, DevTab};
use super::super::tools::{BrushMode, PlacementRejectReason};
use super::state::is_placement_catalog_tab;
use crate::world::{BuildingCatalog, BuildingDefinition, DoodadCatalog, DoodadDefinition};

/// Which catalog placement fields apply for the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlacementControlSet {
    pub pattern: bool,
    pub count: bool,
    pub spacing: bool,
    pub radius: bool,
    pub grid_columns: bool,
    pub grid_rows: bool,
    pub affiliation: bool,
    pub terrain_snap: bool,
    pub preview: bool,
    pub rotation: bool,
    pub scale: bool,
    pub cancel: bool,
    pub footprint_status: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementControlField {
    Pattern,
    Count,
    Spacing,
    Radius,
    GridColumns,
    GridRows,
    Affiliation,
    TerrainSnap,
    Preview,
    Rotation,
    Scale,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementUiContext {
    Hidden,
    EmptyHint,
    Unit,
    Doodad,
    Building,
}

pub fn placement_ui_context(tab: DevTab, dev_state: &DevModeState) -> PlacementUiContext {
    if !is_placement_catalog_tab(tab) {
        return if dev_state.placement_tool_active() {
            PlacementUiContext::EmptyHint
        } else {
            PlacementUiContext::Hidden
        };
    }
    match dev_state.selected_definition.as_ref() {
        None => {
            if dev_state.placement_tool_active() {
                PlacementUiContext::EmptyHint
            } else {
                PlacementUiContext::EmptyHint
            }
        }
        Some(DefinitionId::Unit(_)) => PlacementUiContext::Unit,
        Some(DefinitionId::Doodad(_)) => PlacementUiContext::Doodad,
        Some(DefinitionId::Building(_)) => PlacementUiContext::Building,
        Some(DefinitionId::Item(_) | DefinitionId::InventoryProfile(_)) => {
            PlacementUiContext::Hidden
        }
    }
}

pub fn placement_control_set(
    context: PlacementUiContext,
    brush_mode: BrushMode,
    building: Option<&BuildingDefinition>,
    doodad: Option<&DoodadDefinition>,
) -> PlacementControlSet {
    match context {
        PlacementUiContext::Hidden | PlacementUiContext::EmptyHint => {
            PlacementControlSet::default()
        }
        PlacementUiContext::Unit => unit_controls(brush_mode),
        PlacementUiContext::Doodad => doodad_controls(brush_mode, doodad),
        PlacementUiContext::Building => building_controls(building),
    }
}

fn unit_controls(mode: BrushMode) -> PlacementControlSet {
    let mut set = PlacementControlSet {
        pattern: true,
        affiliation: true,
        terrain_snap: true,
        preview: true,
        cancel: true,
        ..Default::default()
    };
    match mode {
        BrushMode::SingleClick => {}
        BrushMode::Line => {
            set.count = true;
            set.spacing = true;
        }
        BrushMode::Circle | BrushMode::RandomScatter => {
            set.count = true;
            set.radius = true;
        }
        BrushMode::Grid => {
            set.grid_columns = true;
            set.grid_rows = true;
            set.spacing = true;
        }
    }
    set
}

fn doodad_controls(mode: BrushMode, doodad: Option<&DoodadDefinition>) -> PlacementControlSet {
    let mut set = unit_controls(mode);
    set.affiliation = false;
    set.rotation = true;
    set.scale = doodad
        .map(|def| def.min_scale < def.max_scale || def.max_scale > 1.0)
        .unwrap_or(false);
    set
}

fn building_controls(building: Option<&BuildingDefinition>) -> PlacementControlSet {
    let scale_supported = building.is_some_and(|def| def.allow_instance_scale);
    PlacementControlSet {
        preview: true,
        rotation: true,
        scale: scale_supported,
        terrain_snap: true,
        cancel: true,
        footprint_status: true,
        ..Default::default()
    }
}

pub fn placement_status_line(
    dev_state: &DevModeState,
    preview_valid: Option<bool>,
    reject: Option<PlacementRejectReason>,
) -> String {
    if let Some(msg) = dev_state.last_spawn_message.strip_prefix("ERR:") {
        return format!("Cannot place: {msg}");
    }
    if !dev_state.last_spawn_message.is_empty() {
        return dev_state.last_spawn_message.clone();
    }
    if let Some(reason) = reject {
        return format!("Cannot place: {}", reject_reason_label(reason));
    }
    if let Some(valid) = preview_valid {
        if !valid {
            return "Cannot place: invalid position".to_string();
        }
    }
    if dev_state.placement_tool_active() {
        let id = dev_state
            .selected_definition
            .as_ref()
            .map(DefinitionId::id_str)
            .unwrap_or("?");
        return format!("Placement active: {id}");
    }
    String::new()
}

pub fn reject_reason_label(reason: PlacementRejectReason) -> &'static str {
    match reason {
        PlacementRejectReason::TerrainUnavailable => "terrain unavailable",
        PlacementRejectReason::SlopeUnavailable => "slope unavailable",
        PlacementRejectReason::SlopeTooSteep => "slope too steep",
        PlacementRejectReason::BlockedByDoodad => "blocked",
        PlacementRejectReason::TooCloseToPeer => "too close to peer",
    }
}

pub fn building_supports_scale(building_catalog: &BuildingCatalog, id: &DefinitionId) -> bool {
    let DefinitionId::Building(building_id) = id else {
        return false;
    };
    building_catalog
        .get(building_id)
        .is_some_and(|def| def.allow_instance_scale)
}

pub fn doodad_supports_scale(doodad_catalog: &DoodadCatalog, id: &DefinitionId) -> bool {
    let DefinitionId::Doodad(doodad_id) = id else {
        return false;
    };
    doodad_catalog
        .get(doodad_id)
        .map(|def| (def.min_scale - def.max_scale).abs() > f32::EPSILON || def.max_scale > 1.0)
        .unwrap_or(false)
}

/// Hover help for contextual placement controls (Slice 9).
pub fn placement_control_tooltip(field: PlacementControlField) -> &'static str {
    match field {
        PlacementControlField::Pattern => {
            "Cycle brush pattern: single click, line, circle, grid, or random scatter. \
             Pattern determines which spacing/count fields apply."
        }
        PlacementControlField::Count => {
            "Adjust spawn count for line, circle, grid, or scatter brushes."
        }
        PlacementControlField::Spacing => "Distance between spawns in meters (line/grid brushes).",
        PlacementControlField::Radius => "Scatter/circle radius in meters.",
        PlacementControlField::GridColumns | PlacementControlField::GridRows => {
            "Grid dimensions for grid brush placement."
        }
        PlacementControlField::Affiliation => {
            "Cycle spawn team (Player ↔ Wilds). Units only; does not change placed doodads/buildings."
        }
        PlacementControlField::TerrainSnap => {
            "Snap placement to terrain height. Does not bypass slope or overlap validation."
        }
        PlacementControlField::Preview => {
            "Show placement preview ghosts before click. Diagnostic presentation only."
        }
        PlacementControlField::Rotation => "Initial yaw in degrees for the next placement.",
        PlacementControlField::Scale => {
            "Uniform instance scale for the next placement. Hidden when the definition disallows scaling."
        }
        PlacementControlField::Cancel => {
            "Cancel armed placement and clear preview ghosts. Right-click also cancels when no UI is focused."
        }
    }
}
