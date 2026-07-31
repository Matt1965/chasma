//! Save window panel and scene snapshot interaction.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::doodads::DoodadsRuntimeSettings;
use crate::world::{
    BuildingCatalog, FootprintCatalog, InteriorProfileCatalog, UnitCatalog, WorldData,
};

use crate::dev::dev_mode::{DevModeState, DevTextFieldFocus};
use crate::dev::input::DevPanelUi;
use crate::dev::scenes::{
    DevSceneRegistry, SceneDebugFlagsSnapshot, clear_dev_world, delete_scene, load_scene_by_id,
    save_current_world,
};
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::widgets::{
    FIELD_BG_FOCUSED, FIELD_BG_IDLE, FIELD_BORDER_FOCUSED, FIELD_BORDER_IDLE,
    SCENE_NAME_PLACEHOLDER,
};
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};

const MAX_VISIBLE_ROWS: usize = 12;
const ROW_HEIGHT_PX: f32 = 22.0;
const BTN_BG: Color = Color::srgba(0.12, 0.2, 0.28, 0.95);

#[derive(Component, Debug)]
pub struct DevSaveWindowUi;

#[derive(Component, Debug)]
pub(crate) struct DevSaveScenesText;

#[derive(Component, Debug)]
pub(crate) struct DevSaveSceneNameField;

#[derive(Component, Debug)]
pub(crate) struct DevSaveSceneNameText;

#[derive(Component, Debug)]
pub(crate) struct DevSaveSceneClearButton;

#[derive(Component, Debug)]
pub(crate) struct DevSaveSceneListRow {
    pub index: usize,
}

#[derive(Component, Debug)]
pub(crate) struct DevSaveSceneButton {
    action: DevSaveSceneAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevSaveSceneAction {
    SaveCurrent,
    ReloadLast,
    ClearWorld,
    DeleteSelected,
}

pub fn setup_save_window_panel(mut commands: Commands, bodies: Query<(Entity, &DevWindowBody)>) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::Save {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevSaveWindowUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    root.spawn((
                        DevSaveScenesText,
                        DevPanelUi,
                        DevTooltipTarget::new(
                            "Save, load, and manage WorldData snapshot scenes. These are not \
                             full ECS saves. Project-default environment files remain separate.",
                        ),
                        Text::new("Save/load WorldData snapshots (not ECS)"),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
                    ));

                    root.spawn((
                        DevSaveSceneNameField,
                        DevTooltipTarget::new(
                            "Scene name used by Save Current World. Typing here suppresses dev \
                             shortcuts until focus leaves the field.",
                        ),
                        DevPanelUi,
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(24.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(4.0),
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(FIELD_BG_IDLE),
                        BorderColor::all(FIELD_BORDER_IDLE),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            DevSaveSceneNameText,
                            DevPanelUi,
                            Text::new(SCENE_NAME_PLACEHOLDER),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.65, 0.72, 0.80, 1.0)),
                            Node {
                                flex_grow: 1.0,
                                overflow: Overflow::clip(),
                                ..default()
                            },
                        ));
                        row.spawn((
                            DevSaveSceneClearButton,
                            DevPanelUi,
                            Button,
                            Node {
                                width: Val::Px(20.0),
                                height: Val::Px(20.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            Visibility::Hidden,
                            BackgroundColor(Color::srgba(0.18, 0.24, 0.30, 0.9)),
                            Text::new("x"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.85, 0.90, 0.95, 1.0)),
                        ));
                    });

                    for (label, action) in [
                        ("Save Current World", DevSaveSceneAction::SaveCurrent),
                        ("Reload Last Scene", DevSaveSceneAction::ReloadLast),
                        ("Clear World", DevSaveSceneAction::ClearWorld),
                        ("Delete Scene", DevSaveSceneAction::DeleteSelected),
                    ] {
                        root.spawn((
                            DevSaveSceneButton { action },
                            DevPanelUi,
                            DevWindowUi,
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(BTN_BG),
                            Text::new(label),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
                        ));
                    }

                    root.spawn((
                        DevPanelUi,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            max_height: Val::Px(ROW_HEIGHT_PX * MAX_VISIBLE_ROWS as f32),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        for index in 0..MAX_VISIBLE_ROWS {
                            list.spawn((
                                DevSaveSceneListRow { index },
                                DevPanelUi,
                                DevWindowUi,
                                Button,
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(ROW_HEIGHT_PX),
                                    padding: UiRect::horizontal(Val::Px(4.0)),
                                    align_items: AlignItems::Center,
                                    overflow: Overflow::clip(),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.1, 0.14, 0.18, 0.85)),
                                Text::new(""),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::srgba(0.88, 0.92, 0.96, 1.0)),
                            ));
                        }
                    });
                });
        });
        return;
    }
}

pub fn sync_dev_save_panel_visibility(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut ui: Query<&mut Visibility, With<DevSaveWindowUi>>,
) {
    let visible = dev_state.enabled && registry.is_visible(DevWindowId::Save);
    for mut vis in &mut ui {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub fn sync_save_window_content(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    scene_registry: Res<DevSceneRegistry>,
    mut texts: ParamSet<(
        Query<&mut Text, With<DevSaveScenesText>>,
        Query<&mut Text, With<DevSaveSceneNameText>>,
        Query<(
            &DevSaveSceneListRow,
            &Interaction,
            &mut Text,
            &mut BackgroundColor,
        )>,
    )>,
) {
    if !dev_state.enabled || !registry.is_visible(DevWindowId::Save) {
        return;
    }

    if let Ok(mut text) = texts.p0().single_mut() {
        **text = if dev_state.last_scene_message.is_empty() {
            "Save/load WorldData snapshots (not ECS)".into()
        } else {
            dev_state.last_scene_message.clone()
        };
    }

    if let Ok(mut text) = texts.p1().single_mut() {
        let focused = dev_state.text_focus == DevTextFieldFocus::SceneName;
        **text = if dev_state.scene_name_input.is_empty() && !focused {
            SCENE_NAME_PLACEHOLDER.to_string()
        } else {
            dev_state.scene_name_input.clone()
        };
    }

    let scene_entries: Vec<_> = scene_registry
        .registry
        .search(&dev_state.scene_name_input)
        .into_iter()
        .cloned()
        .collect();

    let visible_scenes: Vec<_> = scene_entries
        .into_iter()
        .skip(dev_state.scene_list_scroll)
        .take(MAX_VISIBLE_ROWS)
        .collect();

    for (row, interaction, mut text, mut bg) in texts.p2().iter_mut() {
        if row.index < visible_scenes.len() {
            let entry = &visible_scenes[row.index];
            **text = format!("{}  [{}]", entry.name, entry.scene_id);
            let selected = dev_state.selected_scene_id.as_deref() == Some(entry.scene_id.as_str());
            *bg = if selected {
                BackgroundColor(Color::srgba(0.15, 0.45, 0.32, 0.95))
            } else {
                BackgroundColor(match interaction {
                    Interaction::Pressed => Color::srgba(0.08, 0.12, 0.16, 1.0),
                    Interaction::Hovered => Color::srgba(0.20, 0.30, 0.38, 0.98),
                    Interaction::None => Color::srgba(0.14, 0.22, 0.28, 0.95),
                })
            };
        } else {
            **text = String::new();
            *bg = BackgroundColor(Color::srgba(0.08, 0.1, 0.12, 0.5));
        }
    }
}

pub fn sync_save_window_name_field_style(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut fields: Query<(&mut BackgroundColor, &mut BorderColor), With<DevSaveSceneNameField>>,
    mut name_text: Query<&mut TextColor, With<DevSaveSceneNameText>>,
    mut clear_buttons: Query<&mut Visibility, With<DevSaveSceneClearButton>>,
) {
    if !dev_state.enabled || !registry.is_visible(DevWindowId::Save) {
        return;
    }

    let focused = dev_state.text_focus == DevTextFieldFocus::SceneName;
    for (mut bg, mut border) in &mut fields {
        *bg = BackgroundColor(if focused {
            FIELD_BG_FOCUSED
        } else {
            FIELD_BG_IDLE
        });
        border.set_all(if focused {
            FIELD_BORDER_FOCUSED
        } else {
            FIELD_BORDER_IDLE
        });
    }

    if let Ok(mut color) = name_text.single_mut() {
        *color = TextColor(if focused {
            Color::srgba(0.92, 0.95, 0.98, 1.0)
        } else if dev_state.scene_name_input.is_empty() {
            Color::srgba(0.65, 0.72, 0.80, 1.0)
        } else {
            Color::srgba(0.85, 0.90, 0.95, 1.0)
        });
    }

    let show_clear = !dev_state.scene_name_input.is_empty() && focused;
    for mut visibility in &mut clear_buttons {
        *visibility = if show_clear {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

#[derive(SystemParam)]
pub(crate) struct SaveWindowCatalogResources<'w> {
    unit_catalog: Res<'w, UnitCatalog>,
    doodad_catalog: Res<'w, crate::world::DoodadCatalog>,
    building_catalog: Res<'w, BuildingCatalog>,
    footprint_catalog: Res<'w, FootprintCatalog>,
    interior_catalog: Res<'w, InteriorProfileCatalog>,
    nav_catalog: Res<'w, crate::world::BuildingNavigationBlueprintCatalog>,
}

pub fn handle_save_window_interaction(
    mut dev_state: ResMut<DevModeState>,
    catalogs: SaveWindowCatalogResources,
    mut world: ResMut<WorldData>,
    mut scene_registry: ResMut<DevSceneRegistry>,
    runtime: Option<Res<DoodadsRuntimeSettings>>,
    mut registry: ResMut<DevWindowRegistry>,
    camera_state: Query<&RtsCameraState, With<RtsCamera>>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut buttons: ParamSet<(
        Query<(&Interaction, &DevSaveSceneButton), Changed<Interaction>>,
        Query<(&Interaction, &DevSaveSceneListRow), Changed<Interaction>>,
        Query<&Interaction, (With<DevSaveSceneNameField>, Changed<Interaction>)>,
        Query<&Interaction, (With<DevSaveSceneClearButton>, Changed<Interaction>)>,
        Query<&Interaction, (With<DevSaveWindowUi>, Changed<Interaction>)>,
    )>,
) {
    if !dev_state.enabled || !registry.is_visible(DevWindowId::Save) {
        return;
    }

    for interaction in buttons.p2().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.focus_scene_name();
            registry.focus_window(DevWindowId::Save);
        }
    }

    for interaction in buttons.p3().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            dev_state.scene_name_input.clear();
            dev_state.scene_list_scroll = 0;
            dev_state.focus_scene_name();
        }
    }

    if buttons
        .p4()
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        registry.focus_window(DevWindowId::Save);
    }

    let scene_name_input = dev_state.scene_name_input.clone();
    let scene_list_scroll = dev_state.scene_list_scroll;
    let scene_entries: Vec<_> = scene_registry
        .registry
        .search(&scene_name_input)
        .into_iter()
        .cloned()
        .collect();

    let mut panel_click = false;

    for (interaction, row) in buttons.p1().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        panel_click = true;
        let index = scene_list_scroll + row.index;
        if let Some(entry) = scene_entries.get(index) {
            dev_state.selected_scene_id = Some(entry.scene_id.clone());
            match load_scene_by_id(
                &mut world,
                &catalogs.unit_catalog,
                &catalogs.doodad_catalog,
                &catalogs.building_catalog,
                &catalogs.footprint_catalog,
                &catalogs.interior_catalog,
                Some(&catalogs.nav_catalog),
                &scene_registry.registry,
                &entry.scene_id,
            ) {
                Ok(report) => {
                    dev_state.last_loaded_scene_id = Some(entry.scene_id.clone());
                    dev_state.last_scene_message = format!(
                        "Loaded {} — units={} doodads={} seed={} ({}ms)",
                        entry.name,
                        report.units_loaded,
                        report.doodads_loaded,
                        report.world_seed,
                        report.elapsed_ms
                    );
                }
                Err(err) => {
                    dev_state.last_scene_message = format!("Load failed: {err}");
                }
            }
        }
    }

    for (interaction, button) in buttons.p0().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        panel_click = true;
        apply_scene_action(
            button.action,
            &mut dev_state,
            &mut world,
            &catalogs.unit_catalog,
            &catalogs.doodad_catalog,
            &catalogs.building_catalog,
            &catalogs.footprint_catalog,
            &catalogs.interior_catalog,
            Some(&catalogs.nav_catalog),
            &mut scene_registry,
            runtime.as_deref(),
            camera_state.iter().next(),
        );
    }

    if panel_click {
        dev_state.clear_text_focus();
    }
}

fn apply_scene_action(
    action: DevSaveSceneAction,
    dev_state: &mut DevModeState,
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &crate::world::DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    nav_catalog: Option<&crate::world::BuildingNavigationBlueprintCatalog>,
    scene_registry: &mut DevSceneRegistry,
    runtime: Option<&DoodadsRuntimeSettings>,
    camera: Option<&RtsCameraState>,
) {
    let world_seed = runtime
        .map(|settings| settings.world_seed)
        .unwrap_or(crate::doodads::DEFAULT_DOODAD_WORLD_SEED);
    let debug_flags = Some(SceneDebugFlagsSnapshot::from(dev_state.debug_config));

    match action {
        DevSaveSceneAction::SaveCurrent => {
            let name = if dev_state.scene_name_input.trim().is_empty() {
                "Untitled Scene".to_string()
            } else {
                dev_state.scene_name_input.clone()
            };
            match save_current_world(
                world,
                &mut scene_registry.registry,
                &name,
                world_seed,
                debug_flags,
                camera,
            ) {
                Ok(scene_id) => {
                    dev_state.selected_scene_id = Some(scene_id.clone());
                    dev_state.last_loaded_scene_id = Some(scene_id.clone());
                    dev_state.last_scene_message = format!("Saved scene '{name}' as {scene_id}");
                }
                Err(err) => dev_state.last_scene_message = format!("Save failed: {err}"),
            }
        }
        DevSaveSceneAction::ReloadLast => {
            let Some(scene_id) = dev_state.last_loaded_scene_id.clone() else {
                dev_state.last_scene_message = "No scene loaded yet".into();
                return;
            };
            match load_scene_by_id(
                world,
                unit_catalog,
                doodad_catalog,
                building_catalog,
                footprint_catalog,
                interior_catalog,
                nav_catalog,
                &scene_registry.registry,
                &scene_id,
            ) {
                Ok(report) => {
                    dev_state.last_scene_message = format!(
                        "Reloaded {scene_id} — units={} doodads={} ({}ms)",
                        report.units_loaded, report.doodads_loaded, report.elapsed_ms
                    );
                }
                Err(err) => dev_state.last_scene_message = format!("Reload failed: {err}"),
            }
        }
        DevSaveSceneAction::ClearWorld => {
            clear_dev_world(world);
            dev_state.last_scene_message = "Cleared all units and doodads".into();
        }
        DevSaveSceneAction::DeleteSelected => {
            let Some(scene_id) = dev_state.selected_scene_id.clone() else {
                dev_state.last_scene_message = "Select a scene row first".into();
                return;
            };
            match delete_scene(&mut scene_registry.registry, &scene_id) {
                Ok(()) => {
                    if dev_state.last_loaded_scene_id.as_deref() == Some(scene_id.as_str()) {
                        dev_state.last_loaded_scene_id = None;
                    }
                    dev_state.selected_scene_id = None;
                    dev_state.last_scene_message = format!("Deleted scene {scene_id}");
                }
                Err(err) => dev_state.last_scene_message = format!("Delete failed: {err}"),
            }
        }
    }
}
