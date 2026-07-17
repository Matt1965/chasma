//! Terrain Analysis player panel (ADR-103 TF3).

use bevy::prelude::*;

use crate::terrain::{
    MAX_PLAYER_OVERLAY_OPACITY_BP, TerrainFieldOverlayDiagnostics, TerrainOverlayState,
};
use crate::world::{
    TerrainFieldCatalog, TerrainFieldId, TerrainFieldSample, sample_terrain_field_at,
};
use crate::world::{WorldConfig, WorldData};

use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{BAR_BG, PANEL_BG, TEXT_MUTED, TEXT_PRIMARY};

/// Root node for the Terrain Analysis panel.
#[derive(Component, Debug)]
pub struct TerrainAnalysisRoot;

/// Toggle button in the bottom HUD.
#[derive(Component, Debug)]
pub struct TerrainAnalysisToggleButton;

/// Field selection button.
#[derive(Component, Debug, Clone)]
pub struct TerrainAnalysisFieldButton {
    pub field_id: Option<TerrainFieldId>,
}

#[derive(Component, Debug)]
pub struct TerrainAnalysisLegendText;

#[derive(Component, Debug)]
pub struct TerrainAnalysisCursorText;

#[derive(Component, Debug)]
pub struct TerrainAnalysisOpacityLabel;

pub fn spawn_terrain_analysis_ui(mut commands: Commands) {
    commands
        .spawn((
            TerrainAnalysisRoot,
            PlayerHudUi,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                bottom: Val::Px(210.0),
                width: Val::Px(260.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(6.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR_BG),
            ZIndex(350),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Terrain Analysis"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));
            root.spawn((
                TerrainAnalysisLegendText,
                Text::new("Field: None"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(TEXT_MUTED),
            ));
            root.spawn((
                TerrainAnalysisCursorText,
                Text::new("Cursor: —"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(TEXT_MUTED),
            ));
            root.spawn((
                TerrainAnalysisOpacityLabel,
                Text::new("Opacity: 55%"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(TEXT_MUTED),
            ));
        });

    commands
        .spawn((
            TerrainAnalysisToggleButton,
            PlayerHudUi,
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                bottom: Val::Px(170.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            ZIndex(340),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new("Terrain Analysis"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));
        });
}

pub fn populate_terrain_analysis_field_buttons(
    mut commands: Commands,
    catalog: Res<TerrainFieldCatalog>,
    roots: Query<Entity, With<TerrainAnalysisRoot>>,
) {
    let Ok(root) = roots.single() else {
        return;
    };
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                TerrainAnalysisFieldButton { field_id: None },
                PlayerHudUi,
                Button,
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("None"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(TEXT_PRIMARY),
                ));
            });
        for definition in catalog.definitions() {
            if !definition.enabled || !definition.overlay_style.enabled {
                continue;
            }
            let label = definition.display_name.clone();
            let field_id = definition.id.clone();
            parent
                .spawn((
                    TerrainAnalysisFieldButton {
                        field_id: Some(field_id),
                    },
                    PlayerHudUi,
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(label),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(TEXT_PRIMARY),
                    ));
                });
        }
    });
}

pub fn sync_terrain_analysis_panel(
    overlay_state: Res<TerrainOverlayState>,
    catalog: Res<TerrainFieldCatalog>,
    mut roots: Query<&mut Node, With<TerrainAnalysisRoot>>,
    mut legend: Query<
        &mut Text,
        (
            With<TerrainAnalysisLegendText>,
            Without<TerrainAnalysisCursorText>,
        ),
    >,
    mut cursor_text: Query<
        &mut Text,
        (
            With<TerrainAnalysisCursorText>,
            Without<TerrainAnalysisLegendText>,
        ),
    >,
    mut opacity_label: Query<
        &mut Text,
        (
            With<TerrainAnalysisOpacityLabel>,
            Without<TerrainAnalysisLegendText>,
            Without<TerrainAnalysisCursorText>,
        ),
    >,
) {
    let Ok(mut root_node) = roots.single_mut() else {
        return;
    };
    root_node.display = if overlay_state.panel_open {
        Display::Flex
    } else {
        Display::None
    };

    let Ok(mut legend_text) = legend.single_mut() else {
        return;
    };
    let field_line = match overlay_state.effective_field() {
        Some(id) => catalog
            .get(id)
            .map(|d| format!("Field: {}", d.display_name))
            .unwrap_or_else(|| format!("Field: {id} (missing)")),
        None => "Field: None".to_string(),
    };
    let mut lines = vec![field_line];
    if let Some(id) = overlay_state.effective_field() {
        if let Some(def) = catalog.get(id) {
            let style = &def.overlay_style;
            lines.push(format!(
                "Legend: low→high  cutoff={}",
                style.visibility_cutoff
            ));
            if !style.qualitative_labels.is_empty() {
                lines.push(format!("Bands: {}", style.qualitative_labels.join(" / ")));
            }
            lines.push("Unknown: checker pattern".to_string());
        }
    }
    **legend_text = lines.join("\n");

    if let Ok(mut opacity) = opacity_label.single_mut() {
        **opacity = format!(
            "Opacity: {:.0}%",
            overlay_state.opacity_basis_points as f32 / 100.0
        );
    }

    if let Ok(mut cursor) = cursor_text.single_mut() {
        if !overlay_state.show_cursor_value {
            **cursor = "Cursor: (hidden)".to_string();
        }
    }
}

pub fn handle_terrain_analysis_clicks(
    mut overlay_state: ResMut<TerrainOverlayState>,
    catalog: Res<TerrainFieldCatalog>,
    toggle: Query<&Interaction, (Changed<Interaction>, With<TerrainAnalysisToggleButton>)>,
    field_buttons: Query<
        (&Interaction, &TerrainAnalysisFieldButton),
        (Changed<Interaction>, With<TerrainAnalysisFieldButton>),
    >,
) {
    for interaction in &toggle {
        if *interaction == Interaction::Pressed {
            overlay_state.panel_open = !overlay_state.panel_open;
        }
    }

    for (interaction, button) in &field_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let field = button.field_id.clone();
        if let Some(ref id) = field {
            if catalog.get(id).is_none() || !catalog.get(id).is_some_and(|d| d.enabled) {
                continue;
            }
            if !overlay_state.opacity_user_override {
                if let Some(def) = catalog.get(id) {
                    overlay_state.opacity_basis_points =
                        (def.overlay_style.default_opacity * 10_000.0) as u16;
                }
            }
        }
        overlay_state.set_manual_field(field);
    }
}

pub fn handle_terrain_analysis_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut overlay_state: ResMut<TerrainOverlayState>,
) {
    if keyboard.just_pressed(KeyCode::KeyO) {
        overlay_state.panel_open = !overlay_state.panel_open;
    }
    if !overlay_state.panel_open {
        return;
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        let opacity = overlay_state.opacity_basis_points.saturating_sub(500);
        overlay_state.set_opacity_basis_points(opacity);
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        let opacity = (overlay_state.opacity_basis_points + 500).min(MAX_PLAYER_OVERLAY_OPACITY_BP);
        overlay_state.set_opacity_basis_points(opacity);
    }
}

pub fn update_terrain_analysis_cursor_readout(
    overlay_state: Res<TerrainOverlayState>,
    catalog: Res<TerrainFieldCatalog>,
    world: Res<WorldData>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::RtsCamera>>,
    render_assets: Option<Res<crate::terrain::TerrainRenderAssets>>,
    mut cursor_text: Query<&mut Text, With<TerrainAnalysisCursorText>>,
) {
    if !overlay_state.panel_open || !overlay_state.show_cursor_value {
        return;
    }
    let Ok(mut text) = cursor_text.single_mut() else {
        return;
    };
    let Some(field_id) = overlay_state.effective_field() else {
        **text = "Cursor: —".to_string();
        return;
    };
    let Some(ray) = crate::units::input::cursor_world_ray(&windows, &camera) else {
        **text = "Cursor: —".to_string();
        return;
    };
    let layout = world.layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let Some(click) =
        crate::units::input::terrain_click_to_world_position(&ray, &world, layout, vertical_scale)
    else {
        **text = "Cursor: —".to_string();
        return;
    };
    let sample = sample_terrain_field_at(&world, &catalog, field_id, click.world_position);
    **text = format_cursor_sample(&sample, &catalog);
}

fn format_cursor_sample(sample: &TerrainFieldSample, catalog: &TerrainFieldCatalog) -> String {
    if !sample.availability.is_available() {
        return format!("Cursor: Unknown ({:?})", sample.availability);
    }
    let pct = sample
        .as_percent()
        .map(|p| format!("{p:.1}%"))
        .unwrap_or_else(|| "—".to_string());
    let label = catalog
        .get(&sample.field_id)
        .and_then(|d| d.overlay_style.qualitative_label_for_value(sample.value))
        .unwrap_or("—");
    format!("Cursor: {pct} ({label})  raw={}", sample.value)
}

#[cfg(feature = "dev")]
pub fn sync_terrain_analysis_dev_diagnostics(
    overlay_state: Res<TerrainOverlayState>,
    diagnostics: Res<TerrainFieldOverlayDiagnostics>,
    mut dev_field_state: ResMut<crate::dev::DevTerrainFieldState>,
) {
    dev_field_state.last_action_message = Some(format!(
        "overlay rev={} resident={} uploads={} hits={} missing={} field={:?}",
        diagnostics.last_request_revision,
        diagnostics.resident_overlays,
        diagnostics.uploads,
        diagnostics.cache_hits,
        diagnostics.missing_tiles,
        overlay_state.effective_field()
    ));
}
