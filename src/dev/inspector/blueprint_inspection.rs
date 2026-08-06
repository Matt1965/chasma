//! Building navigation blueprint read-only inspection (NV1.2.5).

use bevy::prelude::*;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::debug::{DebugOverlayConfig, InspectorOverlayFocus};
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::world::{
    BlueprintInspectionValidation, BuildingCatalog, BuildingId, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision, GeometryGenerationDiagnostics, WorldData,
};

use super::capture::capture_building_blueprint_inspection_snapshot;
use super::snapshot::BuildingBlueprintInspectorSnapshot;
use super::state::WorldInspectorState;
use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};

#[derive(Debug, Clone, PartialEq)]
pub enum BlueprintPendingConfirmation {
    ApplyToAsset {
        inheriting_count: usize,
    },
    ResetToAsset,
    RegenerateFromMesh {
        current_source: String,
        destructive: bool,
    },
    ReplaceWorkingCopyWithDraft {
        current_regions: usize,
        current_connections: usize,
        draft_regions: usize,
        draft_connections: usize,
    },
    AdoptGeneratedDraft,
    DiscardEdits {
        action: String,
    },
    ReplaceInstanceWithVariant {
        definition_id: crate::world::BuildingDefinitionId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlueprintVariantDraftField {
    DisplayName,
    AssetId,
    Description,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintVariantDraft {
    pub source_definition_id: crate::world::BuildingDefinitionId,
    pub display_name: String,
    pub asset_id: String,
    pub description: String,
    pub active_field: BlueprintVariantDraftField,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedBlueprintDraft {
    pub blueprint: crate::world::BuildingNavigationBlueprint,
    pub warnings: Vec<String>,
    pub geometry_diagnostics: GeometryGenerationDiagnostics,
    pub mesh_source_label: String,
    pub validation: BlueprintInspectionValidation,
    /// True after the user adopts this draft into the working copy for manual editing.
    pub adopted: bool,
}

/// Session state for blueprint inspection mode (camera save/restore, floor selection).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct BlueprintInspectionState {
    pub active: bool,
    pub editing: bool,
    pub dirty: bool,
    pub building_id: Option<BuildingId>,
    pub selected_floor_id: Option<i32>,
    pub selected_region_key: Option<String>,
    pub focused_diagnostic_index: Option<usize>,
    pub saved_camera: Option<RtsCameraState>,
    pub working_copy: Option<crate::world::BuildingNavigationBlueprint>,
    /// Mesh-generated draft awaiting explicit acceptance (IN-09).
    pub generated_draft: Option<GeneratedBlueprintDraft>,
    pub draft_preview_active: bool,
    /// Working copy before adopting a generated draft (restored by reset / discard adoption).
    pub pre_adoption_working_copy: Option<crate::world::BuildingNavigationBlueprint>,
    pub selection: BlueprintEditSelection,
    pub active_tool: BlueprintEditTool,
    pub drag: Option<BlueprintEditDrag>,
    pub last_pick_message: Option<String>,
    pub pending_confirmation: Option<BlueprintPendingConfirmation>,
    pub variant_draft: Option<BlueprintVariantDraft>,
    /// Two-stage connection authoring: (from_region, to_region pending).
    pub pending_connection_regions: Option<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BlueprintEditTool {
    #[default]
    Select,
    AddVertex,
    AddEntrance,
    AddConnection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BlueprintEditSelection {
    #[default]
    None,
    Region {
        floor_id: i32,
        region_key: String,
    },
    Vertex {
        floor_id: i32,
        region_key: String,
        index: usize,
    },
    Edge {
        floor_id: i32,
        region_key: String,
        index: usize,
    },
    Entrance {
        key: String,
    },
    Transition {
        key: String,
    },
    TransitionTo {
        key: String,
    },
    Connection {
        key: String,
    },
    ConnectionFrom {
        key: String,
    },
    ConnectionTo {
        key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlueprintEditDrag {
    Vertex {
        floor_id: i32,
        region_key: String,
        index: usize,
    },
    Entrance {
        key: String,
    },
    TransitionFrom {
        key: String,
    },
    TransitionTo {
        key: String,
    },
    ConnectionFrom {
        key: String,
    },
    ConnectionTo {
        key: String,
    },
}

impl BlueprintInspectionState {
    /// Ensure [`selected_floor_id`] references a floor in the working blueprint.
    pub fn sync_selected_floor_from_working_copy(&mut self) {
        let Some(working) = self.working_copy.as_ref() else {
            return;
        };
        if working.floors.is_empty() {
            self.selected_floor_id = None;
            return;
        }
        let still_valid = self
            .selected_floor_id
            .is_some_and(|id| working.floors.iter().any(|floor| floor.floor_id == id));
        if still_valid {
            self.sync_selected_region_from_working_copy();
            return;
        }
        let floor = working
            .floors
            .iter()
            .min_by(|a, b| {
                a.elevation_meters
                    .partial_cmp(&b.elevation_meters)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("non-empty floors");
        self.selected_floor_id = Some(floor.floor_id);
        self.sync_selected_region_from_working_copy();
    }

    pub fn sync_selected_region_from_working_copy(&mut self) {
        let Some(working) = self.working_copy.as_ref() else {
            self.selected_region_key = None;
            return;
        };
        let Some(floor_id) = self.selected_floor_id else {
            self.selected_region_key = None;
            return;
        };
        let Some(floor) = working
            .floors
            .iter()
            .find(|floor| floor.floor_id == floor_id)
        else {
            self.selected_region_key = None;
            return;
        };
        if floor.regions.is_empty() {
            self.selected_region_key = None;
            return;
        }
        let still_valid = self
            .selected_region_key
            .as_deref()
            .is_some_and(|key| floor.regions.iter().any(|region| region.key == key));
        if still_valid {
            return;
        }
        if floor.regions.len() == 1 {
            self.selected_region_key = Some(floor.regions[0].key.clone());
        } else {
            self.selected_region_key = None;
        }
    }

    /// Pick an initial region after adopting a generated draft.
    pub fn select_initial_region_after_adoption(
        &mut self,
        validation: &BlueprintInspectionValidation,
    ) {
        let Some(working) = self.working_copy.as_ref() else {
            self.selected_region_key = None;
            return;
        };
        let Some(floor_id) = self.selected_floor_id else {
            self.selected_region_key = None;
            return;
        };
        let Some(floor) = working
            .floors
            .iter()
            .find(|floor| floor.floor_id == floor_id)
        else {
            self.selected_region_key = None;
            return;
        };
        if floor.regions.is_empty() {
            self.selected_region_key = None;
            return;
        }
        if let Some(region_key) =
            region_key_from_validation_diagnostics(validation, floor_id, &floor.regions)
        {
            self.selected_region_key = Some(region_key);
            return;
        }
        if floor.regions.len() == 1 {
            self.selected_region_key = Some(floor.regions[0].key.clone());
        } else {
            self.selected_region_key = None;
        }
    }

    pub fn clear_selection_if_stale(&mut self) {
        let Some(working) = self.working_copy.as_ref() else {
            self.selection = BlueprintEditSelection::None;
            return;
        };
        let valid = match &self.selection {
            BlueprintEditSelection::None => true,
            BlueprintEditSelection::Region {
                floor_id,
                region_key,
            } => working
                .floors
                .iter()
                .find(|floor| floor.floor_id == *floor_id)
                .and_then(|floor| floor.region_by_key(region_key))
                .is_some(),
            BlueprintEditSelection::Vertex {
                floor_id,
                region_key,
                index,
            } => working
                .floors
                .iter()
                .find(|floor| floor.floor_id == *floor_id)
                .and_then(|floor| floor.region_by_key(region_key))
                .is_some_and(|region| *index < region.walkable_outline.vertices_xz.len()),
            BlueprintEditSelection::Edge {
                floor_id,
                region_key,
                index,
            } => working
                .floors
                .iter()
                .find(|floor| floor.floor_id == *floor_id)
                .and_then(|floor| floor.region_by_key(region_key))
                .is_some_and(|region| {
                    let count = region.walkable_outline.vertices_xz.len();
                    count >= 2 && *index < count
                }),
            BlueprintEditSelection::Entrance { key } => working
                .entrances
                .iter()
                .any(|entrance| entrance.key == *key),
            BlueprintEditSelection::Transition { key }
            | BlueprintEditSelection::TransitionTo { key } => working
                .vertical_transitions
                .iter()
                .any(|transition| transition.key == *key),
            BlueprintEditSelection::Connection { key }
            | BlueprintEditSelection::ConnectionFrom { key }
            | BlueprintEditSelection::ConnectionTo { key } => working
                .region_connections
                .iter()
                .any(|connection| connection.key == *key),
        };
        if !valid {
            self.selection = BlueprintEditSelection::None;
        }
    }

    pub fn exit(&mut self) {
        self.active = false;
        self.editing = false;
        self.dirty = false;
        self.building_id = None;
        self.selected_floor_id = None;
        self.selected_region_key = None;
        self.focused_diagnostic_index = None;
        self.saved_camera = None;
        self.working_copy = None;
        self.generated_draft = None;
        self.draft_preview_active = false;
        self.pre_adoption_working_copy = None;
        self.selection = BlueprintEditSelection::None;
        self.active_tool = BlueprintEditTool::Select;
        self.drag = None;
        self.last_pick_message = None;
        self.pending_confirmation = None;
        self.variant_draft = None;
        self.pending_connection_regions = None;
    }

    pub fn has_pending_generated_draft(&self) -> bool {
        self.generated_draft
            .as_ref()
            .is_some_and(|draft| !draft.adopted)
    }

    pub fn is_editing_adopted_draft(&self) -> bool {
        self.generated_draft
            .as_ref()
            .is_some_and(|draft| draft.adopted)
    }

    /// Ensure [`selected_floor_id`] references a floor in the pending generated draft.
    pub fn sync_selected_floor_from_draft(&mut self) {
        let Some(draft) = self.generated_draft.as_ref() else {
            return;
        };
        let floors = &draft.blueprint.floors;
        if floors.is_empty() {
            self.selected_floor_id = None;
            return;
        }
        let still_valid = self
            .selected_floor_id
            .is_some_and(|id| floors.iter().any(|floor| floor.floor_id == id));
        if still_valid {
            return;
        }
        let floor = floors
            .iter()
            .min_by(|a, b| {
                a.elevation_meters
                    .partial_cmp(&b.elevation_meters)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("non-empty floors");
        self.selected_floor_id = Some(floor.floor_id);
    }

    pub fn sync_selected_region_from_draft(&mut self) {
        let Some(draft) = self.generated_draft.as_ref() else {
            return;
        };
        let Some(floor_id) = self.selected_floor_id else {
            self.selected_region_key = None;
            return;
        };
        let Some(floor) = draft
            .blueprint
            .floors
            .iter()
            .find(|floor| floor.floor_id == floor_id)
        else {
            self.selected_region_key = None;
            return;
        };
        if floor.regions.len() == 1 {
            self.selected_region_key = Some(floor.regions[0].key.clone());
        }
    }

    pub fn draft_topology_summary(&self) -> Option<(usize, usize)> {
        self.generated_draft.as_ref().map(|draft| {
            let regions = draft
                .blueprint
                .floors
                .iter()
                .map(|floor| floor.regions.len())
                .sum();
            let connections = draft.blueprint.region_connections.len();
            (regions, connections)
        })
    }

    pub fn working_topology_summary(&self) -> Option<(usize, usize)> {
        self.working_copy.as_ref().map(|blueprint| {
            let regions = blueprint
                .floors
                .iter()
                .map(|floor| floor.regions.len())
                .sum();
            let connections = blueprint.region_connections.len();
            (regions, connections)
        })
    }
}

pub fn adopt_generated_blueprint_draft_for_editing(
    inspection: &mut BlueprintInspectionState,
) -> Result<(), String> {
    let (blueprint, validation) = {
        let draft = inspection
            .generated_draft
            .as_ref()
            .ok_or_else(|| "no generated draft to edit".to_string())?;
        if draft.adopted {
            return Err("generated draft is already being edited".to_string());
        }
        (draft.blueprint.clone(), draft.validation.clone())
    };

    if inspection.pre_adoption_working_copy.is_none() {
        inspection.pre_adoption_working_copy = inspection.working_copy.clone();
    }

    inspection.working_copy = Some(blueprint);
    inspection.dirty = true;
    inspection.editing = true;
    inspection.draft_preview_active = false;
    inspection.selection = BlueprintEditSelection::None;
    inspection.active_tool = BlueprintEditTool::Select;
    inspection.drag = None;
    inspection.pending_connection_regions = None;

    if let Some(draft) = inspection.generated_draft.as_mut() {
        draft.adopted = true;
    }

    inspection.sync_selected_floor_from_working_copy();
    inspection.select_initial_region_after_adoption(&validation);
    Ok(())
}

/// Restore the working copy from before draft adoption (reset / discard adoption edits).
pub fn restore_pre_adoption_working_copy(inspection: &mut BlueprintInspectionState) {
    inspection.working_copy = inspection.pre_adoption_working_copy.take();
    inspection.generated_draft = None;
    inspection.draft_preview_active = false;
    inspection.dirty = false;
    inspection.editing = inspection.working_copy.is_some();
    inspection.selection = BlueprintEditSelection::None;
    inspection.active_tool = BlueprintEditTool::Select;
    inspection.drag = None;
    inspection.pending_connection_regions = None;
    inspection.sync_selected_floor_from_working_copy();
}

pub fn accept_generated_blueprint_draft(
    inspection: &mut BlueprintInspectionState,
) -> Result<(), String> {
    let draft = inspection
        .generated_draft
        .as_ref()
        .ok_or_else(|| "no generated draft to accept".to_string())?;
    if !draft.validation.valid() {
        return Err(format!(
            "generated draft has {} validation errors — fix or discard before accepting",
            draft.validation.error_count
        ));
    }
    let draft = inspection
        .generated_draft
        .take()
        .expect("generated draft present");
    let working = inspection
        .working_copy
        .get_or_insert_with(|| draft.blueprint.clone());
    working.floors = draft.blueprint.floors;
    working.entrances = draft.blueprint.entrances;
    working.vertical_transitions = draft.blueprint.vertical_transitions;
    working.region_connections = draft.blueprint.region_connections;
    inspection.dirty = true;
    inspection.editing = true;
    inspection.draft_preview_active = false;
    inspection.sync_selected_floor_from_working_copy();
    Ok(())
}

pub fn discard_generated_blueprint_draft(inspection: &mut BlueprintInspectionState) {
    if inspection
        .generated_draft
        .as_ref()
        .is_some_and(|draft| draft.adopted)
    {
        return;
    }
    inspection.generated_draft = None;
    inspection.draft_preview_active = false;
}

pub fn format_adopted_draft_status_message(
    validation: &crate::world::BlueprintInspectionValidation,
    region_count: usize,
    connection_count: usize,
) -> String {
    format!(
        "Editing generated draft — unsaved. Regions: {region_count}  Connections: {connection_count}  \
         Validation errors: {}  Warnings: {}",
        validation.error_count, validation.warning_count
    )
}

pub fn format_generated_draft_status_message(draft: &GeneratedBlueprintDraft) -> String {
    if draft.adopted {
        return String::new();
    }
    if draft.validation.valid() {
        return "Generated draft ready for review.".into();
    }
    let headline = draft
        .validation
        .diagnostics
        .iter()
        .find(|diag| diag.level == crate::world::BlueprintDiagnosticLevel::Error)
        .map(|diag| diag.message.clone())
        .unwrap_or_else(|| {
            format!(
                "Generated draft has {} validation errors",
                draft.validation.error_count
            )
        });
    format!("Generated draft contains validation errors. {headline}")
}

pub fn format_fatal_generation_failure_message(error: &str) -> String {
    if error.contains("mesh load failed")
        || error.contains("no triangles")
        || error.contains("no walkable")
        || error.contains("invalid baseline scale")
    {
        format!("Blueprint generation failed before a usable draft could be created. {error}")
    } else {
        format!("Blueprint generation failed: {error}")
    }
}

fn region_key_from_validation_diagnostics(
    validation: &crate::world::BlueprintInspectionValidation,
    floor_id: i32,
    regions: &[crate::world::NavigationRegionDefinition],
) -> Option<String> {
    use crate::world::BlueprintDiagnosticLevel;
    for diag in validation
        .diagnostics
        .iter()
        .filter(|d| d.level == BlueprintDiagnosticLevel::Error)
    {
        if diag
            .focus
            .as_ref()
            .is_some_and(|focus| focus.floor_id == Some(floor_id))
            || diag.focus.is_none()
        {
            if let Some(key) = region_key_from_diagnostic_message(&diag.message) {
                if regions.iter().any(|region| region.key == key) {
                    return Some(key);
                }
            }
        }
    }
    None
}

fn region_key_from_diagnostic_message(message: &str) -> Option<String> {
    message
        .split('`')
        .nth(1)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// Reusable bird's-eye framing for a building anchor and blueprint bounds (NV1.2.5).
pub fn frame_building_for_inspection(
    camera: &mut RtsCameraState,
    building_center: Vec3,
    bounds_half_extent: f32,
    pitch_max: f32,
    distance_min: f32,
    distance_max: f32,
) {
    let padding = 1.35;
    let extent = bounds_half_extent.max(4.0) * padding;
    let distance = (extent * 2.2).clamp(distance_min, distance_max);
    camera.target_focus = building_center;
    camera.target_yaw = 0.0;
    camera.target_pitch = pitch_max * 0.98;
    camera.target_distance = distance;
    camera.focus = building_center;
    camera.yaw = 0.0;
    camera.pitch = pitch_max * 0.98;
    camera.distance = distance;
}

fn blueprint_bounds_half_extent(
    snapshot: &BuildingBlueprintInspectorSnapshot,
    building_center: Vec3,
) -> f32 {
    if snapshot.world_bounds_radius > 0.0 {
        return snapshot.world_bounds_radius;
    }
    8.0 + building_center.xz().length() * 0.0
}

pub fn enter_blueprint_inspection(
    building_id: BuildingId,
    inspection: &mut BlueprintInspectionState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
    snapshot: &BuildingBlueprintInspectorSnapshot,
    pitch_max: f32,
    distance_min: f32,
    distance_max: f32,
    debug_config: &mut DebugOverlayConfig,
) {
    if inspection.saved_camera.is_none() {
        inspection.saved_camera = Some(*camera);
    }
    inspection.active = true;
    inspection.building_id = Some(building_id);
    inspection.selected_floor_id = snapshot
        .floor_ids
        .first()
        .copied()
        .or(snapshot.selected_floor_id);
    inspection.focused_diagnostic_index = None;

    overlay_focus.blueprint_building_id = Some(building_id);
    overlay_focus.blueprint_floor_id = inspection.selected_floor_id;
    overlay_focus.blueprint_diagnostic = None;

    frame_building_for_inspection(
        camera,
        snapshot.building_center,
        blueprint_bounds_half_extent(snapshot, snapshot.building_center),
        pitch_max,
        distance_min,
        distance_max,
    );

    debug_config.nav_blueprint = true;
}

pub fn exit_blueprint_inspection(
    inspection: &mut BlueprintInspectionState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
) {
    if let Some(saved) = inspection.saved_camera.take() {
        *camera = saved;
    }
    inspection.exit();
    overlay_focus.clear_blueprint();
}

pub fn sync_navigation_blueprint_session(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut inspection: ResMut<BlueprintInspectionState>,
    mut inspector: ResMut<WorldInspectorState>,
    world_selection: Res<WorldSelectionState>,
    mut overlay_focus: ResMut<InspectorOverlayFocus>,
    world: Res<WorldData>,
    building_catalog: Res<BuildingCatalog>,
    nav_catalog: Res<BuildingNavigationBlueprintCatalog>,
    mut camera: Query<&mut RtsCameraState, With<RtsCamera>>,
) {
    if !dev_state.enabled || !registry.is_visible(DevWindowId::NavigationEditor) {
        return;
    }

    let building_id = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten();

    let Some(building_id) = building_id else {
        if inspection.active {
            if let Ok(mut cam) = camera.single_mut() {
                exit_blueprint_inspection(&mut inspection, &mut overlay_focus, &mut cam);
            } else {
                inspection.exit();
                overlay_focus.clear_blueprint();
            }
            inspector.blueprint_snapshot = None;
        }
        return;
    };

    if world.get_building(building_id).is_none() {
        if inspection.active {
            if let Ok(mut cam) = camera.single_mut() {
                exit_blueprint_inspection(&mut inspection, &mut overlay_focus, &mut cam);
            } else {
                inspection.exit();
                overlay_focus.clear_blueprint();
            }
            inspector.last_message = format!("Building #{} was removed", building_id.raw());
            inspector.blueprint_snapshot = None;
        }
        return;
    }

    if inspection.building_id != Some(building_id) {
        inspection.building_id = Some(building_id);
    }

    if inspection.active && inspector.blueprint_snapshot.is_none() {
        if let Some(mut snap) = capture_building_blueprint_inspection_snapshot(
            &world,
            &building_catalog,
            &nav_catalog,
            building_id,
            inspection.selected_floor_id,
        ) {
            snap.inspection_active = inspection.active;
            snap.edit_active = inspection.editing;
            if let Some(floor_id) = inspection.selected_floor_id {
                snap = enrich_floor_details(snap, floor_id);
            }
            inspector.blueprint_snapshot = Some(snap);
        }
    } else if inspection.active {
        if let Some(mut snap) = capture_building_blueprint_inspection_snapshot(
            &world,
            &building_catalog,
            &nav_catalog,
            building_id,
            inspection.selected_floor_id,
        ) {
            snap.inspection_active = inspection.active;
            snap.edit_active = inspection.editing;
            snap.edit_dirty = inspection.dirty;
            if let Some(floor_id) = inspection.selected_floor_id {
                snap = enrich_floor_details(snap, floor_id);
            }
            inspector.blueprint_snapshot = Some(snap);
        }
    }
}

/// Legacy name — delegates to [`sync_navigation_blueprint_session`] (Slice 7).
pub fn handle_blueprint_inspection_input(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<DevWindowRegistry>,
    inspection: ResMut<BlueprintInspectionState>,
    inspector: ResMut<WorldInspectorState>,
    world_selection: Res<WorldSelectionState>,
    overlay_focus: ResMut<InspectorOverlayFocus>,
    world: Res<WorldData>,
    building_catalog: Res<BuildingCatalog>,
    nav_catalog: Res<BuildingNavigationBlueprintCatalog>,
    camera: Query<&mut RtsCameraState, With<RtsCamera>>,
) {
    sync_navigation_blueprint_session(
        dev_state,
        registry,
        inspection,
        inspector,
        world_selection,
        overlay_focus,
        world,
        building_catalog,
        nav_catalog,
        camera,
    );
}

/// Capture inspector snapshot using an in-progress editor working copy when provided.
pub fn capture_edit_blueprint_snapshot(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    selected_floor_id: Option<i32>,
    working_override: Option<&crate::world::BuildingNavigationBlueprint>,
) -> Option<BuildingBlueprintInspectorSnapshot> {
    let mut snapshot = capture_building_blueprint_inspection_snapshot(
        world,
        building_catalog,
        nav_catalog,
        building_id,
        selected_floor_id,
    )?;
    if let Some(working) = working_override {
        snapshot.resolved_blueprint = Some(working.clone());
        snapshot.blueprint_id = Some(working.id.as_str().to_string());
        snapshot.validation = crate::world::validate_blueprint_for_inspection(working);
        snapshot.entrance_count = working.entrances.len();
        snapshot.transition_count = working.vertical_transitions.len();
        snapshot.floor_ids = working.floors.iter().map(|floor| floor.floor_id).collect();
        if let Some(floor_id) = selected_floor_id {
            snapshot = enrich_floor_details(snapshot, floor_id);
        }
    }
    Some(snapshot)
}

fn enrich_floor_details(
    mut snap: BuildingBlueprintInspectorSnapshot,
    floor_id: i32,
) -> BuildingBlueprintInspectorSnapshot {
    snap.selected_floor_id = Some(floor_id);
    if let Some(blueprint) = snap.resolved_blueprint.as_ref() {
        if let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == floor_id) {
            snap.selected_floor_vertex_count = floor
                .sole_region_outline()
                .map(|outline| outline.vertices_xz.len())
                .unwrap_or(0);
            snap.selected_floor_elevation = Some(floor.elevation_meters);
            snap.selected_floor_entrances = blueprint
                .entrances
                .iter()
                .filter(|e| e.floor_key == floor.key)
                .map(|e| {
                    format!(
                        "{} @ [{:.1},{:.1}] r={:.1}m",
                        e.key, e.local_position_xz[0], e.local_position_xz[1], e.radius_meters
                    )
                })
                .collect();
            snap.selected_floor_transitions = blueprint
                .vertical_transitions
                .iter()
                .filter(|t| {
                    blueprint
                        .floors
                        .iter()
                        .find(|f| f.key == t.from_floor_key)
                        .map(|f| f.floor_id == floor_id)
                        .unwrap_or(false)
                })
                .map(|t| {
                    format!(
                        "{} {:?} {} → {}",
                        t.key, t.kind, t.from_floor_key, t.to_floor_key
                    )
                })
                .collect();
        }
    }
    snap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bird_eye_frame_sets_overhead_pitch() {
        let mut cam = RtsCameraState::new(Vec3::ZERO, 1.0, 0.5, 100.0);
        frame_building_for_inspection(
            &mut cam,
            Vec3::new(10.0, 0.0, 20.0),
            12.0,
            1.35,
            40.0,
            5000.0,
        );
        assert!((cam.target_pitch - 1.35 * 0.98).abs() < 0.01);
        assert!((cam.target_focus.x - 10.0).abs() < 0.01);
        assert!(cam.target_distance >= 40.0);
    }
}
