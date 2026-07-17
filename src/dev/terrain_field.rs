//! Dev Mode terrain field browser, source inspector, and build actions (ADR-101/102).

use std::path::Path;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::terrain::spawn::TerrainRenderAssets;
use crate::units::input::{cursor_world_ray, terrain_click_to_world_position};
use crate::world::{
    BiomeDependencyRef, BuildDependencies, DEFAULT_TERRAIN_FIELD_MANIFEST_PATH,
    TerrainFieldCatalog, TerrainFieldId, TerrainFieldInterpolationDebug, TerrainFieldSample,
    TerrainFieldSourceProfileCatalog, WorldConfig, WorldData, build_and_package_all_enabled,
    build_and_package_field, sample_terrain_field_at, world_position_to_field_local,
};

use super::DevModeState;
use super::dev_mode::DevTab;
use super::input::DevPanelUi;
use crate::dev::DevModeInputGate;

const FIELD_PACKAGE_DIR: &str = "assets/worlds/main/terrain_fields";

const BTN_BG_IDLE: Color = Color::srgba(0.12, 0.2, 0.28, 0.95);
const BTN_BG_HOVER: Color = Color::srgba(0.18, 0.32, 0.42, 0.98);
const BTN_BG_PRESSED: Color = Color::srgba(0.1, 0.16, 0.22, 1.0);
const BTN_BG_ON: Color = Color::srgba(0.2, 0.55, 0.35, 0.95);
const BTN_BG_ON_HOVER: Color = Color::srgba(0.25, 0.62, 0.42, 0.98);

/// Dev terrain field inspection state (not authoritative).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct DevTerrainFieldState {
    pub probe_enabled: bool,
    pub probe_all_fields: bool,
    pub selected_field: TerrainFieldId,
    pub last_sample: Option<TerrainFieldSample>,
    pub last_interpolation: Option<TerrainFieldInterpolationDebug>,
    pub last_world_position: Option<Vec3>,
    pub show_sample_gizmos: bool,
    pub last_action_message: Option<String>,
}

impl Default for DevTerrainFieldState {
    fn default() -> Self {
        Self {
            probe_enabled: true,
            probe_all_fields: false,
            selected_field: TerrainFieldId::new("water"),
            last_sample: None,
            last_interpolation: None,
            last_world_position: None,
            show_sample_gizmos: false,
            last_action_message: None,
        }
    }
}

#[derive(Component)]
pub(crate) struct DevTerrainFieldSection;

#[derive(Component)]
pub(crate) struct DevTerrainFieldPanelText;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DevTerrainFieldButton {
    pub action: DevTerrainFieldAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DevTerrainFieldAction {
    BuildSelected,
    BuildAll,
    Validate,
    Reload,
    RebuildAssessments,
    CycleField,
    ToggleProbe,
    ToggleGizmos,
}

pub fn setup_dev_terrain_field_state(mut commands: Commands) {
    commands.init_resource::<DevTerrainFieldState>();
}

pub(crate) fn spawn_terrain_field_section(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevTerrainFieldSection,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|section| {
            section.spawn((
                DevTerrainFieldPanelText,
                DevPanelUi,
                Text::new("Terrain Fields"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgba(0.72, 0.88, 0.95, 1.0)),
            ));
            spawn_field_button_row(
                section,
                &[
                    ("Build field", DevTerrainFieldAction::BuildSelected),
                    ("Build all", DevTerrainFieldAction::BuildAll),
                    ("Validate", DevTerrainFieldAction::Validate),
                    ("Reload", DevTerrainFieldAction::Reload),
                    ("Reassess", DevTerrainFieldAction::RebuildAssessments),
                ],
            );
            spawn_field_button_row(
                section,
                &[
                    ("Next field", DevTerrainFieldAction::CycleField),
                    ("Probe", DevTerrainFieldAction::ToggleProbe),
                    ("Gizmos", DevTerrainFieldAction::ToggleGizmos),
                ],
            );
        });
}

fn spawn_field_button_row(
    parent: &mut ChildSpawnerCommands<'_>,
    buttons: &[(&str, DevTerrainFieldAction)],
) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
        ))
        .with_children(|row| {
            for (label, action) in buttons {
                row.spawn((
                    DevTerrainFieldButton { action: *action },
                    DevPanelUi,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(BTN_BG_IDLE),
                    Text::new(*label),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
                ));
            }
        });
}

fn field_button_bg(interaction: &Interaction, active: bool) -> BackgroundColor {
    if active {
        return BackgroundColor(match interaction {
            Interaction::Pressed => BTN_BG_PRESSED,
            Interaction::Hovered => BTN_BG_ON_HOVER,
            Interaction::None => BTN_BG_ON,
        });
    }
    BackgroundColor(match interaction {
        Interaction::Pressed => BTN_BG_PRESSED,
        Interaction::Hovered => BTN_BG_HOVER,
        Interaction::None => BTN_BG_IDLE,
    })
}

pub(crate) fn sync_terrain_field_section_visibility(
    dev_state: Res<DevModeState>,
    mut section: Query<&mut Node, With<DevTerrainFieldSection>>,
) {
    if !dev_state.enabled {
        return;
    }
    let show = dev_state.active_tab == DevTab::TerrainFields;
    if let Ok(mut node) = section.single_mut() {
        node.display = if show { Display::Flex } else { Display::None };
    }
}

pub(crate) fn sync_terrain_field_button_styles(
    dev_state: Res<DevModeState>,
    field_state: Res<DevTerrainFieldState>,
    mut buttons: Query<(&Interaction, &DevTerrainFieldButton, &mut BackgroundColor), With<Button>>,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::TerrainFields {
        return;
    }
    for (interaction, button, mut bg) in &mut buttons {
        let active = match button.action {
            DevTerrainFieldAction::ToggleProbe => field_state.probe_enabled,
            DevTerrainFieldAction::ToggleGizmos => field_state.show_sample_gizmos,
            _ => false,
        };
        *bg = field_button_bg(interaction, active);
    }
}

pub fn sync_dev_terrain_field_panel(
    dev_state: Res<DevModeState>,
    field_state: Res<DevTerrainFieldState>,
    catalog: Res<TerrainFieldCatalog>,
    source_catalog: Res<TerrainFieldSourceProfileCatalog>,
    world: Res<WorldData>,
    mut text: Query<&mut Text, With<DevTerrainFieldPanelText>>,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::TerrainFields {
        return;
    }
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let mut lines = Vec::new();
    lines.push("Terrain Fields".to_string());
    lines.push(format!(
        "Catalog: {} definitions",
        catalog.definitions().len()
    ));
    lines.push(format!(
        "Store revision: {}  memory: {} bytes",
        world.terrain_fields().store_revision(),
        world.terrain_fields().memory_bytes()
    ));
    if let Some(profile) = source_catalog.for_field(&field_state.selected_field) {
        lines.push(format!(
            "Source: {} | {:?} | enabled={}",
            profile.id, profile.source_kind, profile.enabled
        ));
        if let Some(generated) = &profile.generated {
            lines.push(format!(
                "  generator={:?} seed={} deps={:?}",
                generated.generator, generated.world_seed, generated.dependencies
            ));
        }
        if let Some(imported) = &profile.imported {
            lines.push(format!(
                "  asset={} {:?} {:?}",
                imported.asset_path, imported.channel, imported.orientation
            ));
        }
    }
    lines.push("Definitions:".to_string());
    for definition in catalog.definitions() {
        let layer = world.terrain_fields().get_layer(&definition.id);
        let tile_count = layer.map(|l| l.tile_count()).unwrap_or(0);
        lines.push(format!(
            "  {} | {} | {:?} | enabled={} | tiles={}",
            definition.id,
            definition.display_name,
            definition.category,
            definition.enabled,
            tile_count
        ));
    }
    lines.push(format!(
        "Probe: {} field={} all={}",
        if field_state.probe_enabled {
            "on"
        } else {
            "off"
        },
        field_state.selected_field,
        field_state.probe_all_fields
    ));
    if let Some(sample) = &field_state.last_sample {
        lines.push(format!(
            "  availability={:?} value={} pct={:?}",
            sample.availability,
            sample.value,
            sample.as_percent()
        ));
        if let Some(chunk) = sample.chunk {
            lines.push(format!("  chunk=({}, {})", chunk.x, chunk.z));
        }
    }
    if let Some(interp) = &field_state.last_interpolation {
        lines.push(format!(
            "  col={} row={} frac=({}, {}) corners={:?}",
            interp.col, interp.row, interp.frac_x, interp.frac_z, interp.corner_values
        ));
    }
    if let Some(msg) = &field_state.last_action_message {
        lines.push(format!("Action: {msg}"));
    }
    **text = lines.join("\n");
}

pub fn update_dev_terrain_field_probe(
    dev_state: Res<DevModeState>,
    mut field_state: ResMut<DevTerrainFieldState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::RtsCamera>>,
    world: Res<WorldData>,
    catalog: Res<TerrainFieldCatalog>,
    config: Res<WorldConfig>,
    render_assets: Res<TerrainRenderAssets>,
) {
    if !dev_state.enabled
        || dev_state.active_tab != DevTab::TerrainFields
        || !field_state.probe_enabled
    {
        return;
    }
    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = config.chunk_layout();
    let Some(click) =
        terrain_click_to_world_position(&ray, &world, layout, render_assets.vertical_scale)
    else {
        return;
    };
    let position = click.world_position;
    field_state.last_world_position = Some(position.to_global(layout));
    let sample = sample_terrain_field_at(&world, &catalog, &field_state.selected_field, position);
    field_state.last_sample = Some(sample);
    if let Ok((_, local)) = world_position_to_field_local(position, layout) {
        if let Some(tile) = world
            .terrain_fields()
            .get_tile(&field_state.selected_field, position.chunk)
        {
            if let Ok((_, debug)) = crate::world::bilinear_sample_u16(tile, local) {
                field_state.last_interpolation = Some(debug);
            }
        }
    }
}

pub fn draw_dev_terrain_field_gizmos(
    dev_state: Res<DevModeState>,
    field_state: Res<DevTerrainFieldState>,
    world: Res<WorldData>,
    catalog: Res<TerrainFieldCatalog>,
    config: Res<WorldConfig>,
    mut gizmos: Gizmos,
) {
    if !dev_state.enabled
        || dev_state.active_tab != DevTab::TerrainFields
        || !field_state.show_sample_gizmos
    {
        return;
    }
    let Some(layer) = world
        .terrain_fields()
        .get_layer(&field_state.selected_field)
    else {
        return;
    };
    let layout = config.chunk_layout();
    let chunk_size = layout.chunk_size_units();
    let spacing = crate::world::TERRAIN_FIELD_SAMPLE_SPACING_METERS;
    let mut drawn = 0usize;
    const MAX_GIZMO_MARKERS: usize = 256;
    for tile in layer.tiles.values() {
        let origin_x = tile.chunk.x as f32 * chunk_size;
        let origin_z = tile.chunk.z as f32 * chunk_size;
        for row in (0..tile.samples_per_edge).step_by(4) {
            for col in (0..tile.samples_per_edge).step_by(4) {
                if drawn >= MAX_GIZMO_MARKERS {
                    return;
                }
                let x = origin_x + col as f32 * spacing;
                let z = origin_z + row as f32 * spacing;
                let value = tile.sample_at_vertex(col as u32, row as u32).unwrap_or(0);
                let t = value as f32 / 65_535.0;
                let color = Color::srgba(t, 0.2, 1.0 - t, 0.85);
                gizmos.sphere(
                    Isometry3d::from_translation(Vec3::new(x, 0.5, z)),
                    0.35,
                    color,
                );
                drawn += 1;
            }
        }
    }
    let _ = catalog;
}

fn build_dependencies<'a>(world: &'a WorldData) -> BuildDependencies<'a> {
    BuildDependencies {
        heightfield: None,
        biome: world.biome_mask().map(|mask| BiomeDependencyRef { mask }),
        terrain_manifest_path: None,
    }
}

fn assessment_catalogs<'a>(
    building_catalog: &'a crate::world::BuildingCatalog,
    requirement_catalog: &'a crate::world::BuildingFieldRequirementCatalog,
    profile_catalog: &'a crate::world::FieldResponseProfileCatalog,
    catalog: &'a TerrainFieldCatalog,
    footprint_catalog: &'a crate::world::FootprintCatalog,
    requirement_revision: u64,
    profile_revision: u64,
) -> crate::world::TerrainAssessmentCatalogs<'a> {
    crate::world::TerrainAssessmentCatalogs {
        buildings: building_catalog,
        requirements: requirement_catalog,
        profiles: profile_catalog,
        fields: catalog,
        footprints: footprint_catalog,
        requirement_revision,
        profile_revision,
    }
}

fn cycle_selected_field(field_state: &mut DevTerrainFieldState, catalog: &TerrainFieldCatalog) {
    let ids = catalog.sorted_ids();
    if ids.is_empty() {
        return;
    }
    let current = ids
        .iter()
        .position(|id| id == &field_state.selected_field)
        .unwrap_or(0);
    let next = (current + 1) % ids.len();
    field_state.selected_field = ids[next].clone();
}

pub(crate) fn handle_terrain_field_buttons(
    dev_state: Res<DevModeState>,
    mut gate: ResMut<DevModeInputGate>,
    mut field_state: ResMut<DevTerrainFieldState>,
    mut world: ResMut<WorldData>,
    catalog: Res<TerrainFieldCatalog>,
    source_catalog: Res<TerrainFieldSourceProfileCatalog>,
    config: Res<WorldConfig>,
    building_catalog: Res<crate::world::BuildingCatalog>,
    footprint_catalog: Res<crate::world::FootprintCatalog>,
    requirement_catalog: Res<crate::world::BuildingFieldRequirementCatalog>,
    profile_catalog: Res<crate::world::FieldResponseProfileCatalog>,
    requirement_revision: Res<crate::world::BuildingFieldRequirementCatalogRevision>,
    profile_revision: Res<crate::world::FieldResponseProfileCatalogRevision>,
    mut assessments: ResMut<crate::world::BuildingTerrainAssessmentStore>,
    buttons: Query<(&Interaction, &DevTerrainFieldButton), Changed<Interaction>>,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::TerrainFields {
        return;
    }

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;

        match button.action {
            DevTerrainFieldAction::CycleField => {
                cycle_selected_field(&mut field_state, &catalog);
            }
            DevTerrainFieldAction::ToggleProbe => {
                field_state.probe_enabled = !field_state.probe_enabled;
            }
            DevTerrainFieldAction::ToggleGizmos => {
                field_state.show_sample_gizmos = !field_state.show_sample_gizmos;
            }
            DevTerrainFieldAction::Validate => {
                let Some(profile) = source_catalog.for_field(&field_state.selected_field) else {
                    field_state.last_action_message =
                        Some("no source profile for field".to_string());
                    continue;
                };
                field_state.last_action_message = Some(match profile.validate() {
                    Ok(()) => format!("valid: {}", profile.id),
                    Err(err) => format!("invalid: {err}"),
                });
            }
            DevTerrainFieldAction::Reload => {
                let assessment_catalogs = assessment_catalogs(
                    &building_catalog,
                    &requirement_catalog,
                    &profile_catalog,
                    &catalog,
                    &footprint_catalog,
                    requirement_revision.0,
                    profile_revision.0,
                );
                field_state.last_action_message =
                    match crate::world::reload_terrain_fields_with_invalidation(
                        &mut world,
                        &catalog,
                        &config,
                        &assessment_catalogs,
                        &mut assessments,
                        Path::new(DEFAULT_TERRAIN_FIELD_MANIFEST_PATH),
                    ) {
                        Ok((summary, diff, rebuild)) => Some(format!(
                            "reloaded {} tiles; {} field changes; reassessed {}",
                            summary.tiles_loaded,
                            diff.changed_tiles.len(),
                            rebuild.assessed
                        )),
                        Err(err) => Some(format!("reload failed: {err}")),
                    };
            }
            DevTerrainFieldAction::RebuildAssessments => {
                let assessment_catalogs = assessment_catalogs(
                    &building_catalog,
                    &requirement_catalog,
                    &profile_catalog,
                    &catalog,
                    &footprint_catalog,
                    requirement_revision.0,
                    profile_revision.0,
                );
                let report = crate::world::rebuild_all_building_terrain_assessments(
                    &world,
                    &assessment_catalogs,
                    &mut assessments,
                );
                field_state.last_action_message = Some(format!(
                    "rebuilt {} assessments ({} skipped, {} failed)",
                    report.assessed,
                    report.skipped_no_requirements,
                    report.failures.len()
                ));
            }
            DevTerrainFieldAction::BuildSelected | DevTerrainFieldAction::BuildAll => {
                let Some(extent) = world.extent() else {
                    field_state.last_action_message = Some("no authored world extent".to_string());
                    continue;
                };
                let deps = build_dependencies(&world);
                let output = Path::new(FIELD_PACKAGE_DIR);
                let result = if button.action == DevTerrainFieldAction::BuildAll {
                    build_and_package_all_enabled(
                        source_catalog.profiles(),
                        extent,
                        &config,
                        output,
                        "main",
                        &deps,
                    )
                    .map(|(reports, package)| {
                        format!(
                            "built {} fields, {} tiles, version={}",
                            reports.len(),
                            package.tiles_written,
                            package.source_version
                        )
                    })
                } else {
                    let Some(profile) = source_catalog.for_field(&field_state.selected_field)
                    else {
                        field_state.last_action_message = Some("no source profile".to_string());
                        continue;
                    };
                    build_and_package_field(profile, extent, &config, output, "main", &deps).map(
                        |(report, package)| {
                            format!(
                                "built {} tiles min={} max={} avg={:.0} version={}",
                                package.tiles_written,
                                report.statistics.minimum,
                                report.statistics.maximum,
                                report.statistics.average,
                                report.source_version
                            )
                        },
                    )
                };
                field_state.last_action_message = Some(match result {
                    Ok(msg) => msg,
                    Err(err) => format!("build failed: {err}"),
                });
            }
        }
    }
}
