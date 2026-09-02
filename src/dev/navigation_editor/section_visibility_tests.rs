//! Regression tests for Navigation Editor collapsible section ownership.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use super::panel::DevNavigationEditorSectionHeader;
use super::state::NavigationEditorUiState;
use super::sync_panel::sync_navigation_editor_section_visibility;
use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::dev_mode::DevModeState;
use crate::dev::inspector::BlueprintInspectionState;
use crate::dev::inspector::BuildingBlueprintInspectorSnapshot;
use crate::dev::inspector::WorldInspectorState;
use crate::dev::widgets::{DevCollapsibleSection, DevCollapsibleSectionId};
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::world::{
    BuildingId, BuildingNavigationBlueprint, two_room_hut_navigation_blueprint,
    validate_blueprint_for_inspection,
};

fn nav_sync_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::ui::UiPlugin));
    app.init_resource::<DevModeState>()
        .init_resource::<DevWindowRegistry>()
        .init_resource::<WorldSelectionState>()
        .init_resource::<WorldInspectorState>()
        .init_resource::<BlueprintInspectionState>()
        .init_resource::<NavigationEditorUiState>();
    app
}

fn spawn_section(world: &mut World, id: DevCollapsibleSectionId, display: Display) -> Entity {
    world
        .spawn((
            DevCollapsibleSection { id },
            Node {
                display,
                ..default()
            },
        ))
        .id()
}

fn spawn_nav_header(world: &mut World, display: Display) -> Entity {
    world
        .spawn((
            DevNavigationEditorSectionHeader,
            Node {
                display,
                ..default()
            },
        ))
        .id()
}

fn run_nav_section_sync(app: &mut App) {
    app.world_mut()
        .run_system_once(sync_navigation_editor_section_visibility)
        .expect("sync_navigation_editor_section_visibility");
}

fn section_display(app: &App, entity: Entity) -> Display {
    app.world()
        .get::<Node>(entity)
        .expect("section node")
        .display
}

struct NavSyncContext {
    dev_enabled: bool,
    nav_visible: bool,
    building_selected: bool,
    generation_details_expanded: bool,
    regeneration_source_label: Option<String>,
    validation_diag_count: usize,
}

fn apply_nav_sync_context(app: &mut App, ctx: &NavSyncContext) {
    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = ctx.dev_enabled;
    }
    {
        let mut registry = app.world_mut().resource_mut::<DevWindowRegistry>();
        if ctx.nav_visible {
            registry.show(DevWindowId::NavigationEditor);
        } else {
            registry.hide(DevWindowId::NavigationEditor);
        }
    }
    {
        let mut selection = app.world_mut().resource_mut::<WorldSelectionState>();
        if ctx.building_selected {
            selection.category = WorldSelectionCategory::Building;
            selection.building_id = Some(BuildingId::new(1));
        } else {
            selection.category = WorldSelectionCategory::None;
            selection.building_id = None;
        }
    }
    {
        let mut ui = app.world_mut().resource_mut::<NavigationEditorUiState>();
        ui.generation_details_expanded = ctx.generation_details_expanded;
        ui.regeneration_source_label = ctx.regeneration_source_label.clone();
        ui.generation_diagnostics = None;
    }
    if ctx.validation_diag_count > 0 {
        let mut blueprint = two_room_hut_navigation_blueprint();
        blueprint.floors.clear();
        let validation = validate_blueprint_for_inspection(&blueprint);
        assert!(
            !validation.diagnostics.is_empty(),
            "test fixture requires a blueprint validation diagnostic"
        );
        let mut inspector = app.world_mut().resource_mut::<WorldInspectorState>();
        inspector.blueprint_snapshot = Some(BuildingBlueprintInspectorSnapshot {
            validation,
            blueprint_id: None,
            blueprint_source: String::new(),
            generator_version: 0,
            generation_status: String::new(),
            cache_fresh: false,
            source_fingerprint: None,
            floor_ids: Vec::new(),
            selected_floor_id: None,
            selected_floor_vertex_count: 0,
            selected_floor_elevation: None,
            selected_floor_entrances: Vec::new(),
            selected_floor_transitions: Vec::new(),
            entrance_count: 0,
            transition_count: 0,
            inspection_active: false,
            edit_active: false,
            edit_dirty: false,
            selected_element: None,
            variant_draft_active: false,
            variant_draft_display_name: None,
            variant_draft_asset_id: None,
            variant_draft_description: None,
            variant_draft_active_field: None,
            building_center: Vec3::ZERO,
            world_bounds_radius: 0.0,
            resolved_blueprint: None,
        });
    } else {
        app.world_mut()
            .resource_mut::<WorldInspectorState>()
            .blueprint_snapshot = None;
    }
}

#[test]
fn nav_section_sync_leaves_fields_build_unchanged_without_building() {
    let mut app = nav_sync_test_app();
    let fields = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::FieldsBuild,
        Display::Flex,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: false,
            generation_details_expanded: false,
            regeneration_source_label: None,
            validation_diag_count: 0,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(
        section_display(&app, fields),
        Display::Flex,
        "FieldsBuild must not be mutated when Navigation Editor sync runs without a building"
    );
}

#[test]
fn nav_section_sync_leaves_foreign_section_display_untouched() {
    let mut app = nav_sync_test_app();
    let world_harness = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::WorldHarness,
        Display::None,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: true,
            generation_details_expanded: true,
            regeneration_source_label: Some("occupancy_collision".into()),
            validation_diag_count: 1,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(
        section_display(&app, world_harness),
        Display::None,
        "foreign sections must remain exactly as authored"
    );
}

#[test]
fn nav_section_sync_hides_generation_without_building() {
    let mut app = nav_sync_test_app();
    let generation = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::NavEditorGeneration,
        Display::Flex,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: false,
            generation_details_expanded: true,
            regeneration_source_label: None,
            validation_diag_count: 0,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, generation), Display::None);
}

#[test]
fn nav_section_sync_shows_generation_when_details_expanded() {
    let mut app = nav_sync_test_app();
    let generation = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::NavEditorGeneration,
        Display::None,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: true,
            generation_details_expanded: true,
            regeneration_source_label: None,
            validation_diag_count: 0,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, generation), Display::Flex);
}

#[test]
fn nav_section_sync_shows_generation_when_summary_present() {
    let mut app = nav_sync_test_app();
    let generation = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::NavEditorGeneration,
        Display::None,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: true,
            generation_details_expanded: false,
            regeneration_source_label: Some("occupancy_collision".into()),
            validation_diag_count: 0,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, generation), Display::Flex);
}

#[test]
fn nav_section_sync_hides_generation_without_summary_or_expansion() {
    let mut app = nav_sync_test_app();
    let generation = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::NavEditorGeneration,
        Display::Flex,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: true,
            generation_details_expanded: false,
            regeneration_source_label: None,
            validation_diag_count: 0,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, generation), Display::None);
}

#[test]
fn nav_section_sync_hides_validation_without_diagnostics() {
    let mut app = nav_sync_test_app();
    let validation = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::NavEditorValidation,
        Display::Flex,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: true,
            generation_details_expanded: false,
            regeneration_source_label: None,
            validation_diag_count: 0,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, validation), Display::None);
}

#[test]
fn nav_section_sync_shows_validation_with_diagnostics() {
    let mut app = nav_sync_test_app();
    let validation = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::NavEditorValidation,
        Display::None,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: true,
            generation_details_expanded: false,
            regeneration_source_label: None,
            validation_diag_count: 1,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, validation), Display::Flex);
}

#[test]
fn nav_section_sync_hides_nav_sections_when_editor_closed() {
    let mut app = nav_sync_test_app();
    let generation = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::NavEditorGeneration,
        Display::Flex,
    );
    let validation = spawn_section(
        app.world_mut(),
        DevCollapsibleSectionId::NavEditorValidation,
        Display::Flex,
    );
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: false,
            building_selected: true,
            generation_details_expanded: true,
            regeneration_source_label: Some("occupancy_collision".into()),
            validation_diag_count: 1,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, generation), Display::None);
    assert_eq!(section_display(&app, validation), Display::None);
}

#[test]
fn nav_section_sync_header_follows_building_selection() {
    let mut app = nav_sync_test_app();
    let header = spawn_nav_header(app.world_mut(), Display::Flex);
    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: false,
            generation_details_expanded: false,
            regeneration_source_label: None,
            validation_diag_count: 0,
        },
    );

    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, header), Display::None);

    apply_nav_sync_context(
        &mut app,
        &NavSyncContext {
            dev_enabled: true,
            nav_visible: true,
            building_selected: true,
            generation_details_expanded: false,
            regeneration_source_label: None,
            validation_diag_count: 0,
        },
    );
    run_nav_section_sync(&mut app);

    assert_eq!(section_display(&app, header), Display::Flex);
}
