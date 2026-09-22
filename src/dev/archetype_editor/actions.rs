use std::path::Path;

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::units::input::SelectedUnits;
use crate::world::{
    build_building_archetype_definition, build_unit_archetype_definition,
    capture_building_archetype_members, capture_unit_archetype_template,
    default_building_archetype_capture_margin_meters, save_building_archetype_catalog_to_ron,
    save_unit_archetype_catalog_to_ron, unique_building_archetype_id, unique_unit_archetype_id,
    validate_building_archetype_definition, validate_gold_range, BuildingArchetypeCatalog,
    BuildingCatalog, BuildingRecord, DoodadCatalog, FootprintCatalog, ItemCatalog,
    OperationCatalog, UnitArchetypeCatalog, UnitArchetypeId, WorldData, BUILDING_ARCHETYPES_RON_PATH,
    UNIT_ARCHETYPES_RON_PATH,
};

use super::capture_preview::{
    BuildingArchetypeMemberPreviewEntry, parse_capture_margin_input,
};
use super::state::{ArchetypeEditorMode, DevArchetypeEditorState};

#[derive(Resource, Debug, Default)]
pub struct DevArchetypeEditorScratch {
    pub pending_building: Option<BuildingRecord>,
    pub capture_margin_input: String,
    pub preview_region: Option<crate::world::BuildingArchetypeCaptureRegion>,
    pub preview_members: Vec<BuildingArchetypeMemberPreviewEntry>,
}

impl DevArchetypeEditorScratch {
    pub fn clear_capture_preview(&mut self) {
        self.preview_region = None;
        self.preview_members.clear();
    }

    pub fn clear_building_capture_session(&mut self) {
        self.pending_building = None;
        self.capture_margin_input.clear();
        self.clear_capture_preview();
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevArchetypeSaveButton;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevArchetypeEditButton;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DevArchetypeModalSaveButton;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DevArchetypeModalDeleteButton;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DevArchetypeModalCancelButton;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeSpeciesToggle(pub crate::world::SpeciesId);

pub fn handle_archetype_save_button(
    mut editor: ResMut<DevArchetypeEditorState>,
    mut scratch: ResMut<DevArchetypeEditorScratch>,
    mut dev_state: ResMut<crate::dev::dev_mode::DevModeState>,
    world: Res<WorldData>,
    item_catalog: Res<ItemCatalog>,
    unit_archetypes: Res<UnitArchetypeCatalog>,
    building_archetypes: Res<BuildingArchetypeCatalog>,
    selected_units: Res<SelectedUnits>,
    world_selection: Res<WorldSelectionState>,
    interaction: Query<&Interaction, With<DevArchetypeSaveButton>>,
) {
    if interaction.iter().all(|i| *i != Interaction::Pressed) {
        return;
    }

    let selected_unit_archetype = dev_state.selected_unit_archetype.clone();
    let selected_building_archetype = dev_state.selected_building_archetype.clone();
    match dev_state.active_tab {
        crate::dev::dev_mode::DevTab::Units => begin_unit_save_flow(
            &mut editor,
            &mut dev_state,
            &world,
            &item_catalog,
            &unit_archetypes,
            &selected_units,
            &world_selection,
            selected_unit_archetype,
        ),
        crate::dev::dev_mode::DevTab::Buildings => begin_building_save_flow(
            &mut editor,
            &mut scratch,
            &mut dev_state,
            &world,
            &building_archetypes,
            &world_selection,
            selected_building_archetype,
        ),
        _ => {}
    }
}

pub fn handle_archetype_edit_button(
    mut editor: ResMut<DevArchetypeEditorState>,
    mut dev_state: ResMut<crate::dev::dev_mode::DevModeState>,
    unit_archetypes: Res<UnitArchetypeCatalog>,
    building_archetypes: Res<BuildingArchetypeCatalog>,
    interaction: Query<&Interaction, With<DevArchetypeEditButton>>,
) {
    if interaction.iter().all(|i| *i != Interaction::Pressed) {
        return;
    }

    match dev_state.active_tab {
        crate::dev::dev_mode::DevTab::Units => {
            let Some(id) = dev_state.selected_unit_archetype.clone() else {
                dev_state.last_spawn_message = "Select an archetype to edit.".to_string();
                return;
            };
            let Some(definition) = unit_archetypes.get(&id) else {
                dev_state.last_spawn_message = "Selected archetype no longer exists.".to_string();
                return;
            };
            editor.modal_open = true;
            editor.mode = Some(ArchetypeEditorMode::UnitEdit);
            editor.editing_unit_id = Some(id);
            editor.name_input = definition.display_name.clone();
            editor.gold_min_input = definition.gold_min.to_string();
            editor.gold_max_input = definition.gold_max.to_string();
            editor.selected_species = definition.applicable_species.iter().cloned().collect();
            editor.load_dialogue_from_definition(definition.dialogue.as_ref());
            editor.captured_unit_template = None;
            editor.pending_template_update = false;
            editor.status_message.clear();
        }
        crate::dev::dev_mode::DevTab::Buildings => {
            let Some(id) = dev_state.selected_building_archetype.clone() else {
                dev_state.last_spawn_message = "Select an archetype to edit.".to_string();
                return;
            };
            let Some(definition) = building_archetypes.get(&id) else {
                dev_state.last_spawn_message = "Selected archetype no longer exists.".to_string();
                return;
            };
            editor.modal_open = true;
            editor.mode = Some(ArchetypeEditorMode::BuildingEdit);
            editor.editing_building_id = Some(id);
            editor.name_input = definition.display_name.clone();
            editor.status_message.clear();
        }
        _ => {}
    }
}

pub fn handle_archetype_modal_cancel(
    mut editor: ResMut<DevArchetypeEditorState>,
    mut scratch: ResMut<DevArchetypeEditorScratch>,
    mut dev_state: ResMut<crate::dev::dev_mode::DevModeState>,
    interaction: Query<&Interaction, With<DevArchetypeModalCancelButton>>,
) {
    if interaction.iter().all(|i| *i != Interaction::Pressed) {
        return;
    }
    editor.close();
    scratch.clear_building_capture_session();
    dev_state.clear_text_focus();
}

pub fn handle_archetype_modal_delete(
    mut editor: ResMut<DevArchetypeEditorState>,
    mut dev_state: ResMut<crate::dev::dev_mode::DevModeState>,
    mut unit_archetypes: ResMut<UnitArchetypeCatalog>,
    mut building_archetypes: ResMut<BuildingArchetypeCatalog>,
    interaction: Query<&Interaction, With<DevArchetypeModalDeleteButton>>,
) {
    if interaction.iter().all(|i| *i != Interaction::Pressed) {
        return;
    }
    match editor.mode {
        Some(ArchetypeEditorMode::UnitEdit) => {
            let Some(id) = editor.editing_unit_id.clone() else {
                return;
            };
            unit_archetypes.remove(&id);
            let _ = save_unit_archetype_catalog_to_ron(
                &unit_archetypes,
                Path::new(UNIT_ARCHETYPES_RON_PATH),
            );
            if dev_state.selected_unit_archetype.as_ref() == Some(&id) {
                dev_state.selected_unit_archetype = None;
            }
            dev_state.last_spawn_message = format!("Deleted unit archetype `{}`.", id.as_str());
        }
        Some(ArchetypeEditorMode::BuildingEdit) => {
            let Some(id) = editor.editing_building_id.clone() else {
                return;
            };
            building_archetypes.remove(&id);
            let _ = save_building_archetype_catalog_to_ron(
                &building_archetypes,
                Path::new(BUILDING_ARCHETYPES_RON_PATH),
            );
            if dev_state.selected_building_archetype.as_ref() == Some(&id) {
                dev_state.selected_building_archetype = None;
            }
            dev_state.last_spawn_message = format!("Deleted building archetype `{}`.", id.as_str());
        }
        _ => return,
    }
    editor.close();
    dev_state.clear_text_focus();
}

pub fn handle_archetype_modal_save(
    mut editor: ResMut<DevArchetypeEditorState>,
    mut scratch: ResMut<DevArchetypeEditorScratch>,
    mut dev_state: ResMut<crate::dev::dev_mode::DevModeState>,
    mut unit_archetypes: ResMut<UnitArchetypeCatalog>,
    mut building_archetypes: ResMut<BuildingArchetypeCatalog>,
    world: Res<WorldData>,
    building_catalog: Res<BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    item_catalog: Res<ItemCatalog>,
    operation_catalog: Res<OperationCatalog>,
    selected_units: Res<SelectedUnits>,
    world_selection: Res<WorldSelectionState>,
    interaction: Query<&Interaction, With<DevArchetypeModalSaveButton>>,
) {
    if interaction.iter().all(|i| *i != Interaction::Pressed) {
        return;
    }

    let name = editor.name_input.trim();
    if name.is_empty() {
        editor.status_message = "Name is required.".to_string();
        return;
    }

    match editor.mode {
        Some(ArchetypeEditorMode::UnitCreate) | Some(ArchetypeEditorMode::UnitEdit) => {
            save_unit_archetype_from_modal(
                &mut editor,
                &mut dev_state,
                &mut unit_archetypes,
                &world,
                &item_catalog,
                &selected_units,
                &world_selection,
            );
        }
        Some(ArchetypeEditorMode::BuildingCreate) | Some(ArchetypeEditorMode::BuildingEdit) => {
            save_building_archetype_from_modal(
                &mut editor,
                &mut scratch,
                &mut dev_state,
                &mut building_archetypes,
                &world,
                &building_catalog,
                &footprint_catalog,
                &doodad_catalog,
                &item_catalog,
                &operation_catalog,
            );
        }
        None => {}
    }
}

pub fn handle_archetype_dialogue_toggle(
    mut editor: ResMut<DevArchetypeEditorState>,
    interaction: Query<(&Interaction, &super::modal::DevArchetypeDialogueToggle), Changed<Interaction>>,
) {
    if !editor.modal_open || !editor.is_unit_modal() {
        return;
    }
    for (state, toggle) in &interaction {
        if *state != Interaction::Pressed {
            continue;
        }
        match toggle {
            super::modal::DevArchetypeDialogueToggle::Talk => {
                editor.dialogue_talk_enabled = !editor.dialogue_talk_enabled;
            }
            super::modal::DevArchetypeDialogueToggle::Trade => {
                editor.dialogue_trade_enabled = !editor.dialogue_trade_enabled;
            }
            super::modal::DevArchetypeDialogueToggle::Recruit => {
                editor.dialogue_recruit_enabled = !editor.dialogue_recruit_enabled;
            }
        }
    }
}

pub fn handle_archetype_species_toggle(
    mut editor: ResMut<DevArchetypeEditorState>,
    interaction: Query<(&Interaction, &DevArchetypeSpeciesToggle), Changed<Interaction>>,
) {
    if !editor.modal_open || !editor.is_unit_modal() {
        return;
    }
    for (state, toggle) in &interaction {
        if *state != Interaction::Pressed {
            continue;
        }
        if editor.selected_species.contains(&toggle.0) {
            editor.selected_species.remove(&toggle.0);
        } else {
            editor.selected_species.insert(toggle.0.clone());
        }
    }
}

fn begin_unit_save_flow(
    editor: &mut DevArchetypeEditorState,
    dev_state: &mut crate::dev::dev_mode::DevModeState,
    world: &WorldData,
    item_catalog: &ItemCatalog,
    unit_archetypes: &UnitArchetypeCatalog,
    selected_units: &SelectedUnits,
    world_selection: &WorldSelectionState,
    selected_archetype: Option<UnitArchetypeId>,
) {
    let unit_id = match require_single_selected_unit(selected_units, world_selection) {
        Ok(id) => id,
        Err(message) => {
            dev_state.last_spawn_message = message;
            return;
        }
    };
    let unit = match world.get_unit(unit_id) {
        Some(record) => record.clone(),
        None => {
            dev_state.last_spawn_message = "Selected unit no longer exists.".to_string();
            return;
        }
    };
    let template = match capture_unit_archetype_template(world, &unit, item_catalog) {
        Ok(template) => template,
        Err(err) => {
            dev_state.last_spawn_message = format!("Could not capture unit template: {err:?}");
            return;
        }
    };

    if let Some(existing_id) = selected_archetype {
        editor.modal_open = true;
        editor.mode = Some(ArchetypeEditorMode::UnitEdit);
        editor.editing_unit_id = Some(existing_id.clone());
        if let Some(definition) = unit_archetypes.get(&existing_id) {
            editor.name_input = definition.display_name.clone();
            editor.gold_min_input = definition.gold_min.to_string();
            editor.gold_max_input = definition.gold_max.to_string();
            editor.selected_species = definition.applicable_species.iter().cloned().collect();
        }
        editor.captured_unit_template = Some(template);
        editor.pending_template_update = true;
        editor.status_message =
            "Updating template from selected unit. Adjust metadata and confirm.".to_string();
        return;
    }

    editor.modal_open = true;
    editor.mode = Some(ArchetypeEditorMode::UnitCreate);
    editor.editing_unit_id = None;
    editor.name_input.clear();
    editor.gold_min_input = template.gold_on_unit.to_string();
    editor.gold_max_input = template.gold_on_unit.to_string();
    editor.selected_species = std::collections::HashSet::from([template.species_id.clone()]);
    editor.captured_unit_template = Some(template);
    editor.pending_template_update = false;
    editor.status_message.clear();
}

fn begin_building_save_flow(
    editor: &mut DevArchetypeEditorState,
    scratch: &mut DevArchetypeEditorScratch,
    dev_state: &mut crate::dev::dev_mode::DevModeState,
    world: &WorldData,
    building_archetypes: &BuildingArchetypeCatalog,
    world_selection: &WorldSelectionState,
    selected_archetype: Option<crate::world::BuildingArchetypeId>,
) {
    scratch.clear_capture_preview();
    let building_id = match require_single_selected_building(world_selection) {
        Ok(id) => id,
        Err(message) => {
            dev_state.last_spawn_message = message;
            return;
        }
    };
    let building = match world.get_building(building_id) {
        Some(record) => record.clone(),
        None => {
            dev_state.last_spawn_message = "Selected building no longer exists.".to_string();
            return;
        }
    };

    if let Some(existing_id) = selected_archetype {
        editor.modal_open = true;
        editor.mode = Some(ArchetypeEditorMode::BuildingEdit);
        editor.editing_building_id = Some(existing_id.clone());
        editor.name_input = building_archetypes
            .get(&existing_id)
            .map(|definition| definition.display_name.clone())
            .unwrap_or_else(|| existing_id.as_str().to_string());
        scratch.pending_building = Some(building);
        scratch.capture_margin_input = building_archetypes
            .get(&existing_id)
            .map(|definition| definition.capture_metadata.capture_margin_meters.to_string())
            .unwrap_or_else(|| default_capture_margin_input());
        editor.pending_template_update = true;
        editor.status_message =
            "Updating template from selected building. Confirm to save.".to_string();
        return;
    }

    editor.modal_open = true;
    editor.mode = Some(ArchetypeEditorMode::BuildingCreate);
    editor.editing_building_id = None;
    editor.name_input.clear();
    scratch.pending_building = Some(building);
    scratch.capture_margin_input = default_capture_margin_input();
    editor.pending_template_update = false;
    editor.status_message.clear();
}

fn save_unit_archetype_from_modal(
    editor: &mut DevArchetypeEditorState,
    dev_state: &mut crate::dev::dev_mode::DevModeState,
    unit_archetypes: &mut UnitArchetypeCatalog,
    world: &WorldData,
    item_catalog: &ItemCatalog,
    selected_units: &SelectedUnits,
    world_selection: &WorldSelectionState,
) {
    let name = editor.name_input.trim();
    if editor.selected_species.is_empty() {
        editor.status_message = "Select at least one applicable species.".to_string();
        return;
    }
    let gold_min = parse_u32_field(&editor.gold_min_input, "gold min");
    let gold_max = parse_u32_field(&editor.gold_max_input, "gold max");
    let (gold_min, gold_max) = match (gold_min, gold_max) {
        (Ok(min), Ok(max)) => (min, max),
        (Err(message), _) | (_, Err(message)) => {
            editor.status_message = message;
            return;
        }
    };
    if let Err(message) = validate_gold_range(gold_min, gold_max) {
        editor.status_message = message;
        return;
    }

    let template = if editor.pending_template_update || editor.captured_unit_template.is_none() {
        let unit_id = match require_single_selected_unit(selected_units, world_selection) {
            Ok(id) => id,
            Err(message) => {
                editor.status_message = message;
                return;
            }
        };
        let unit = match world.get_unit(unit_id) {
            Some(record) => record.clone(),
            None => {
                editor.status_message = "Selected unit no longer exists.".to_string();
                return;
            }
        };
        match capture_unit_archetype_template(world, &unit, item_catalog) {
            Ok(template) => template,
            Err(err) => {
                editor.status_message = format!("Could not capture unit template: {err:?}");
                return;
            }
        }
    } else {
        editor.captured_unit_template.clone().unwrap()
    };

    let id = match &editor.editing_unit_id {
        Some(existing) => existing.clone(),
        None => unique_unit_archetype_id(name, unit_archetypes),
    };

    if editor.mode == Some(ArchetypeEditorMode::UnitCreate)
        && unit_archetypes.display_name_taken(name, None)
    {
        editor.status_message = "An archetype with this name already exists.".to_string();
        return;
    }

    let definition = build_unit_archetype_definition(
        id.clone(),
        name.to_string(),
        editor.selected_species.iter().cloned().collect(),
        gold_min,
        gold_max,
        &template,
        editor.build_dialogue_config(),
    );

    if unit_archetypes.upsert(definition).is_err() {
        editor.status_message = "Failed to save unit archetype.".to_string();
        return;
    }
    if save_unit_archetype_catalog_to_ron(unit_archetypes, Path::new(UNIT_ARCHETYPES_RON_PATH)).is_err()
    {
        editor.status_message = "Saved in memory but failed to write RON file.".to_string();
        return;
    }

    dev_state.selected_unit_archetype = Some(id);
    dev_state.last_spawn_message = format!("Saved unit archetype `{name}`.");
    editor.close();
    dev_state.clear_text_focus();
}

fn save_building_archetype_from_modal(
    editor: &mut DevArchetypeEditorState,
    scratch: &mut DevArchetypeEditorScratch,
    dev_state: &mut crate::dev::dev_mode::DevModeState,
    building_archetypes: &mut BuildingArchetypeCatalog,
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    item_catalog: &ItemCatalog,
    operation_catalog: &OperationCatalog,
) {
    let name = editor.name_input.trim();
    let id = match &editor.editing_building_id {
        Some(existing) => existing.clone(),
        None => unique_building_archetype_id(name, building_archetypes),
    };

    if editor.mode == Some(ArchetypeEditorMode::BuildingCreate)
        && building_archetypes.display_name_taken(name, None)
    {
        editor.status_message = "An archetype with this name already exists.".to_string();
        return;
    }

    let definition = if let Some(building) = scratch.pending_building.clone() {
        let margin = parse_capture_margin_input(&scratch.capture_margin_input);
        let (capture_metadata, members) = match capture_building_archetype_members(
            world,
            &building,
            building_catalog,
            footprint_catalog,
            doodad_catalog,
            margin,
        ) {
            Ok(result) => result,
            Err(err) => {
                editor.status_message = format!("Could not capture building archetype: {err:?}");
                return;
            }
        };
        let enabled = building_archetypes
            .get(&id)
            .map(|existing| existing.enabled)
            .unwrap_or(true);
        build_building_archetype_definition(
            id.clone(),
            name.to_string(),
            &building,
            world,
            capture_metadata,
            members,
            enabled,
        )
    } else if let Some(existing) = building_archetypes.get(&id).cloned() {
        let mut definition = existing;
        definition.display_name = name.to_string();
        definition
    } else {
        editor.status_message = "No building archetype to update.".to_string();
        return;
    };
    if let Err(error) = validate_building_archetype_definition(
        &definition,
        item_catalog,
        building_catalog,
        doodad_catalog,
        operation_catalog,
    ) {
        editor.status_message = format!("Invalid building archetype snapshot: {error:?}");
        return;
    }
    if building_archetypes.upsert(definition).is_err() {
        editor.status_message = "Failed to save building archetype.".to_string();
        return;
    }
    if save_building_archetype_catalog_to_ron(building_archetypes, Path::new(BUILDING_ARCHETYPES_RON_PATH))
        .is_err()
    {
        editor.status_message = "Saved in memory but failed to write RON file.".to_string();
        return;
    }

    dev_state.selected_building_archetype = Some(id);
    dev_state.last_spawn_message = format!("Saved building archetype `{name}`.");
    scratch.clear_building_capture_session();
    editor.close();
    dev_state.clear_text_focus();
}

fn default_capture_margin_input() -> String {
    default_building_archetype_capture_margin_meters().to_string()
}

fn require_single_selected_unit(
    selected_units: &SelectedUnits,
    world_selection: &WorldSelectionState,
) -> Result<crate::world::UnitId, String> {
    if world_selection.category != WorldSelectionCategory::Units {
        return Err("Select exactly one unit in the world to save an archetype.".to_string());
    }
    if selected_units.is_empty() {
        return Err("Select exactly one unit to save an archetype.".to_string());
    }
    if selected_units.0.len() != 1 {
        return Err("Select exactly one unit. Multiple units are selected.".to_string());
    }
    Ok(*selected_units.0.iter().next().expect("single unit"))
}

fn require_single_selected_building(
    world_selection: &WorldSelectionState,
) -> Result<crate::world::BuildingId, String> {
    if world_selection.category != WorldSelectionCategory::Building {
        return Err("Select exactly one building in the world to save an archetype.".to_string());
    }
    world_selection
        .building_id
        .ok_or_else(|| "Select exactly one building to save an archetype.".to_string())
}

fn parse_u32_field(raw: &str, label: &str) -> Result<u32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    trimmed
        .parse::<u32>()
        .map_err(|_| format!("Invalid {label} value."))
}
