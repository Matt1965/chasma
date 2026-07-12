//! Dev mode keyboard, spawn clicks, and gameplay input gating (ADR-043/044/047, DV2).

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::doodads::DoodadsRuntimeSettings;
use crate::simulation::SimulationControlState;
use crate::terrain::TerrainRenderAssets;
use crate::units::input::{BoxSelectDrag, cursor_world_ray, terrain_click_to_world_position};
use crate::world::{DoodadCatalog, UnitCatalog, WorldConfig, WorldData};

use super::catalog_cache::DevSearchDebounce;
use super::dev_mode::{DevModeInputGate, DevModeState, DevTab, DevTextFieldFocus};
use super::history::DevSpawnRecord;
use super::spawn_tools::dev_spawn_position_from_terrain_click;
use super::tools::{
    BatchSpawnRequest, BatchSpawnScratch, DevPlacementPreview, DevPreviewAnchor,
    execute_batch_spawn,
};

const SHIFT_BATCH_COUNT: u32 = 5;

/// Reset input gate at the start of each frame.
pub fn reset_dev_input_gate(mut gate: ResMut<DevModeInputGate>) {
    gate.reset();
}

/// Cancel armed placement tool and clear preview ghosts.
pub fn cancel_dev_placement(dev_state: &mut DevModeState, preview: &mut DevPlacementPreview) {
    dev_state.cancel_placement_tool();
    preview.clear();
}

/// F12 toggle, search typing, tab shortcuts, favorites, and quick-spawn hotkeys.
pub fn dev_mode_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dev_state: ResMut<DevModeState>,
    mut debounce: ResMut<DevSearchDebounce>,
) {
    if keyboard.just_pressed(KeyCode::F12) {
        dev_state.toggle();
    }

    if !dev_state.enabled {
        return;
    }

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    if keyboard.just_pressed(KeyCode::Slash) || (ctrl && keyboard.just_pressed(KeyCode::KeyF)) {
        if dev_state.active_tab == DevTab::Scenes {
            dev_state.focus_scene_name();
        } else {
            dev_state.focus_catalog_search();
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::Tab) && !dev_state.has_text_focus() {
        dev_state.active_tab = match dev_state.active_tab {
            DevTab::Units => DevTab::Doodads,
            DevTab::Doodads => DevTab::Placement,
            DevTab::Placement => DevTab::Scenes,
            DevTab::Scenes => DevTab::Inspector,
            DevTab::Inspector => DevTab::Debug,
            DevTab::Debug => DevTab::WorldTools,
            DevTab::WorldTools => DevTab::Units,
        };
        dev_state.list_scroll = 0;
    }

    if dev_state.has_text_focus() {
        handle_text_field_input(&keyboard, &mut dev_state, &mut debounce);
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyE) {
        dev_state.enabled_only = !dev_state.enabled_only;
    }

    if keyboard.just_pressed(KeyCode::KeyT) {
        dev_state.cycle_spawn_affiliation();
    }

    handle_favorite_hotkeys(&keyboard, &mut dev_state);
}

/// Esc cancels placement tool and clears search focus (DV2).
pub fn handle_dev_tool_cancel_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    panel_hovered: Res<DevPanelHoverState>,
    mut dev_state: ResMut<DevModeState>,
    mut preview: ResMut<DevPlacementPreview>,
    mut gate: ResMut<DevModeInputGate>,
) {
    if !dev_state.enabled {
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        let had_tool = dev_state.placement_tool_active();
        dev_state.clear_text_focus();
        if had_tool {
            cancel_dev_placement(&mut dev_state, &mut preview);
            gate.block_gameplay_mouse = true;
        }
        return;
    }

    if panel_hovered.hovered {
        return;
    }

    if mouse_buttons.just_pressed(MouseButton::Right) && dev_state.placement_tool_active() {
        cancel_dev_placement(&mut dev_state, &mut preview);
        gate.block_gameplay_mouse = true;
    }
}

fn handle_text_field_input(
    keyboard: &ButtonInput<KeyCode>,
    dev_state: &mut DevModeState,
    debounce: &mut DevSearchDebounce,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        dev_state.clear_text_focus();
        return;
    }

    match dev_state.text_focus {
        DevTextFieldFocus::SceneName => {
            if keyboard.just_pressed(KeyCode::Backspace) {
                dev_state.scene_name_input.pop();
                dev_state.scene_list_scroll = 0;
            }
            for key in keyboard.get_just_pressed() {
                if let Some(ch) = key_to_search_char(*key) {
                    dev_state.scene_name_input.push(ch);
                    dev_state.scene_list_scroll = 0;
                }
            }
        }
        DevTextFieldFocus::CatalogSearch => {
            if keyboard.just_pressed(KeyCode::Backspace) {
                dev_state.search_query.pop();
                dev_state.list_scroll = 0;
                debounce.note_input(&dev_state.search_query);
            }
            if keyboard.just_pressed(KeyCode::KeyF) {
                if let Some(id) = dev_state.selected_definition.clone() {
                    dev_state.toggle_favorite(id);
                }
            }
            for key in keyboard.get_just_pressed() {
                if let Some(ch) = key_to_search_char(*key) {
                    dev_state.search_query.push(ch);
                    dev_state.list_scroll = 0;
                    debounce.note_input(&dev_state.search_query);
                }
            }
        }
        DevTextFieldFocus::None => {}
    }
}

fn handle_favorite_hotkeys(keyboard: &ButtonInput<KeyCode>, dev_state: &mut DevModeState) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    for slot in 0..9 {
        let key = digit_key(slot);
        if !keyboard.just_pressed(key) {
            continue;
        }
        if ctrl {
            if let Some(id) = dev_state.selected_definition.clone() {
                dev_state.assign_favorite_slot(slot, id);
            }
        } else if let Some(id) = dev_state.favorite_slot(slot).cloned() {
            dev_state.select_definition(id);
        }
    }

    if keyboard.just_pressed(KeyCode::KeyF) {
        if let Some(id) = dev_state.selected_definition.clone() {
            dev_state.toggle_favorite(id);
        }
    }
}

fn digit_key(slot: usize) -> KeyCode {
    match slot {
        0 => KeyCode::Digit1,
        1 => KeyCode::Digit2,
        2 => KeyCode::Digit3,
        3 => KeyCode::Digit4,
        4 => KeyCode::Digit5,
        5 => KeyCode::Digit6,
        6 => KeyCode::Digit7,
        7 => KeyCode::Digit8,
        _ => KeyCode::Digit9,
    }
}

fn key_to_search_char(key: KeyCode) -> Option<char> {
    match key {
        KeyCode::Minus => Some('-'),
        KeyCode::Period => Some('.'),
        KeyCode::Slash => Some('/'),
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
        _ => None,
    }
}

/// Whether the dev panel is under the cursor (blocks gameplay mouse).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DevPanelHoverState {
    pub hovered: bool,
}

/// Track panel hover from UI interaction states.
pub fn update_dev_panel_hover_state(
    dev_state: Res<DevModeState>,
    interactions: Query<&Interaction, With<DevPanelUi>>,
    mut hover: ResMut<DevPanelHoverState>,
) {
    hover.hovered =
        dev_state.enabled && interactions.iter().any(|state| *state != Interaction::None);
}

/// Update terrain anchor under cursor for brush preview.
pub fn update_dev_preview_anchor(
    panel_hovered: Res<DevPanelHoverState>,
    mut dev_state: ResMut<DevModeState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut anchor: ResMut<DevPreviewAnchor>,
) {
    if !dev_state.enabled || panel_hovered.hovered {
        return;
    }
    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    if let Some(click) = terrain_click_to_world_position(&ray, &world, layout, vertical_scale) {
        if let Some(grounded) = dev_spawn_position_from_terrain_click(&world, click.world_position)
        {
            anchor.position = grounded;
            let dir = Vec3::new(ray.direction.x, 0.0, ray.direction.z);
            if dir.length_squared() > 1e-6 {
                let flat = dir.normalize();
                dev_state.last_line_direction = Vec2::new(flat.x, flat.z);
            }
        }
    }
}

/// Left-click terrain batch spawn when a definition is selected (before gameplay input).
pub fn handle_dev_spawn_click(
    panel_hovered: Res<DevPanelHoverState>,
    mut gate: ResMut<DevModeInputGate>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    box_drag: Res<BoxSelectDrag>,
    mut world: ResMut<WorldData>,
    config: Res<WorldConfig>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    runtime: Option<Res<DoodadsRuntimeSettings>>,
    simulation: Res<SimulationControlState>,
    mut dev_state: ResMut<DevModeState>,
    mut batch_scratch: Local<BatchSpawnScratch>,
) {
    if !dev_state.enabled {
        return;
    }

    if panel_hovered.hovered {
        gate.block_gameplay_mouse = true;
        return;
    }

    if gate.spawn_handled_this_frame {
        return;
    }

    if !mouse_buttons.just_pressed(MouseButton::Left) || box_drag.is_box_drag() {
        return;
    }

    dev_state.clear_text_focus();

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    let definition = if ctrl {
        dev_state
            .last_spawn
            .as_ref()
            .map(|(id, _)| id.clone())
            .or_else(|| dev_state.selected_definition.clone())
    } else {
        dev_state.selected_definition.clone()
    };

    let Some(definition) = definition else {
        return;
    };

    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    let Some(click) = terrain_click_to_world_position(&ray, &world, layout, vertical_scale) else {
        dev_state.last_spawn_message = "Terrain raycast missed".to_string();
        gate.block_gameplay_mouse = true;
        gate.spawn_handled_this_frame = true;
        return;
    };

    let Some(position) = dev_spawn_position_from_terrain_click(&world, click.world_position) else {
        dev_state.last_spawn_message = "Ground query failed".to_string();
        gate.block_gameplay_mouse = true;
        gate.spawn_handled_this_frame = true;
        return;
    };

    let dir = Vec3::new(ray.direction.x, 0.0, ray.direction.z);
    if dir.length_squared() > 1e-6 {
        let flat = dir.normalize();
        dev_state.last_line_direction = Vec2::new(flat.x, flat.z);
    }

    let mut brush = dev_state.brush;
    if shift {
        brush.count = brush.count.max(SHIFT_BATCH_COUNT);
    }

    let request = BatchSpawnRequest {
        definition: definition.clone(),
        brush,
        anchor: position,
        line_direction: dev_state.last_line_direction,
        terrain_conforming: dev_state.terrain_conforming,
        rules: dev_state.placement_rules,
        world_seed: runtime
            .as_ref()
            .map(|r| r.world_seed)
            .unwrap_or(crate::doodads::DEFAULT_DOODAD_WORLD_SEED),
        layout,
        spawn_affiliation: dev_state.spawn_affiliation,
    };

    let report = execute_batch_spawn(
        &request,
        definition.id_str(),
        &mut world,
        &unit_catalog,
        &doodad_catalog,
        &mut batch_scratch,
    );

    gate.block_gameplay_mouse = true;
    gate.spawn_handled_this_frame = true;

    if report.spawned > 0 {
        let spawn_type = dev_state.spawn_mode;
        dev_state.last_spawn = Some((definition.clone(), position));
        dev_state.spawn_history.push(DevSpawnRecord {
            definition,
            position,
            spawn_type,
            simulation_tick: simulation.current_tick,
        });
    }

    dev_state.last_spawn_message = format!(
        "Batch spawn: {} placed, {} rejected, {} failed ({} attempted)",
        report.spawned, report.rejected, report.failures, report.attempted
    );
}

/// Marker for all dev panel UI nodes.
#[derive(Component, Debug)]
pub struct DevPanelUi;

/// Root dev panel entity.
#[derive(Component, Debug)]
pub struct DevPanelRoot;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::dev_mode::{DefinitionId, SpawnMode};
    use crate::world::WorldPosition;

    #[test]
    fn f12_toggles_dev_mode_state() {
        let mut state = DevModeState::default();
        assert!(!state.enabled);
        state.toggle();
        assert!(state.enabled);
        state.toggle();
        assert!(!state.enabled);
    }

    #[test]
    fn disabled_dev_mode_ignores_spawn_selection() {
        let state = DevModeState::default();
        assert!(!state.enabled);
        assert!(state.selected_definition.is_none());
    }

    #[test]
    fn tab_switch_preserves_selection() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.select_definition(DefinitionId::Unit(crate::world::UnitDefinitionId::new(
            "wolf",
        )));
        state.active_tab = DevTab::Doodads;
        assert!(state.selected_definition.is_some());
    }

    #[test]
    fn favorite_slots_assign_and_recall() {
        let mut state = DevModeState::default();
        let id = DefinitionId::Unit(crate::world::UnitDefinitionId::new("wolf"));
        state.assign_favorite_slot(0, id.clone());
        assert_eq!(state.favorite_slot(0), Some(&id));
    }

    #[test]
    fn spawn_history_records_last_entry() {
        use crate::world::{ChunkCoord, LocalPosition};
        use bevy::prelude::Vec3;

        let mut state = DevModeState::default();
        let id = DefinitionId::Doodad(crate::world::DoodadDefinitionId::new("tree"));
        state.spawn_history.push(DevSpawnRecord {
            definition: id.clone(),
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(1.0, 0.0, 2.0)),
            ),
            spawn_type: SpawnMode::Doodad,
            simulation_tick: 42,
        });
        assert_eq!(state.spawn_history.last().unwrap().simulation_tick, 42);
        assert_eq!(
            state.spawn_history.last().unwrap().definition.id_str(),
            "tree"
        );
    }

    #[test]
    fn reset_tool_state_keeps_enabled_flag() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.search_query = "wolf".into();
        state.reset_tool_state();
        assert!(state.enabled);
        assert!(state.search_query.is_empty());
    }

    #[test]
    fn text_focus_suppresses_global_shortcut_gate() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.focus_catalog_search();
        assert!(state.has_text_focus());
        state.clear_text_focus();
        assert!(!state.has_text_focus());
    }

    #[test]
    fn cancel_placement_clears_selection_and_preview() {
        let mut state = DevModeState::default();
        state.select_definition(DefinitionId::Unit(crate::world::UnitDefinitionId::new(
            "wolf",
        )));
        let mut preview = DevPlacementPreview::default();
        preview.active = true;
        cancel_dev_placement(&mut state, &mut preview);
        assert!(!state.placement_tool_active());
        assert!(!preview.active);
    }

    #[test]
    fn search_query_survives_tab_switch() {
        let mut state = DevModeState::default();
        state.search_query = "oak".into();
        state.active_tab = DevTab::Doodads;
        assert_eq!(state.search_query, "oak");
    }
}
