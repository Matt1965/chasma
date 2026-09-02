//! Fields window panel construction and interaction tests.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use crate::dev::dev_mode::{DevModeInputGate, DevModeState};
use crate::dev::fields_window::panel::DevFieldsWindowUi;
use crate::dev::terrain_field::{
    DevTerrainFieldAction, DevTerrainFieldButton, DevTerrainFieldSection, DevTerrainFieldState,
    handle_terrain_field_buttons, sync_terrain_field_button_styles,
};
use crate::dev::widgets::{DevCollapsibleBody, DevCollapsibleSection, DevCollapsibleSectionId};
use crate::dev::window::{DevWindowId, DevWindowRegistry, setup_dev_workspace};
use crate::dev::{
    setup_dev_terrain_field_state, setup_fields_window_panel, sync_dev_fields_panel_visibility,
};
use crate::terrain::field_overlay::{TerrainFieldAuxiliaryOverlays, TerrainOverlayState};
use crate::world::{
    BuildingCatalog, BuildingFieldRequirementCatalog, BuildingFieldRequirementCatalogRevision,
    BuildingTerrainAssessmentStore, FieldResponseProfileCatalog,
    FieldResponseProfileCatalogRevision, FootprintCatalog, TerrainFieldCatalog,
    TerrainFieldSourceProfileCatalog, WorldConfig, WorldData,
};

fn headless_world_ui_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::ui::UiPlugin));
    app.init_resource::<DevWindowRegistry>()
        .init_resource::<DevModeState>()
        .init_resource::<DevModeInputGate>()
        .init_resource::<crate::dev::widgets::DevCollapsibleState>()
        .init_resource::<DevTerrainFieldState>()
        .init_resource::<TerrainFieldAuxiliaryOverlays>();
    app
}

#[test]
fn dev_mode_startup_chain_populates_fields_window() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_fields_window_panel)
        .expect("setup_fields_window_panel");
    app.world_mut()
        .run_system_once(setup_dev_terrain_field_state)
        .expect("setup_dev_terrain_field_state");

    let mut world = app.world_mut();
    let sections = world
        .query::<&DevTerrainFieldSection>()
        .iter(&mut world)
        .count();
    assert_eq!(
        sections, 1,
        "dev Startup chain should attach terrain field controls to Fields window"
    );
}

#[test]
fn fields_panel_spawns_terrain_field_controls() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_fields_window_panel)
        .expect("setup_fields_window_panel");
    app.world_mut()
        .run_system_once(setup_dev_terrain_field_state)
        .expect("setup_dev_terrain_field_state");

    let mut world = app.world_mut();
    let sections: Vec<_> = world
        .query::<&DevTerrainFieldSection>()
        .iter(&mut world)
        .collect();
    assert_eq!(
        sections.len(),
        1,
        "expected terrain field section in Fields window"
    );

    let build_buttons: Vec<_> = world
        .query::<&DevTerrainFieldButton>()
        .iter(&mut world)
        .filter(|button| button.action == DevTerrainFieldAction::BuildSelected)
        .collect();
    assert_eq!(build_buttons.len(), 1, "expected Build field button");

    let collapsible: Vec<_> = world
        .query::<&DevCollapsibleSection>()
        .iter(&mut world)
        .filter(|section| section.id == DevCollapsibleSectionId::FieldsBuild)
        .collect();
    assert_eq!(
        collapsible.len(),
        1,
        "expected FieldsBuild collapsible section"
    );
}

#[test]
fn fields_panel_body_contains_graphical_button_labels() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_fields_window_panel)
        .expect("setup_fields_window_panel");

    let mut world = app.world_mut();
    let labels: Vec<String> = world
        .query::<&Text>()
        .iter(&mut world)
        .map(|text| text.to_string())
        .collect();
    assert!(
        labels.iter().any(|label| label == "Build field"),
        "expected Build field label among {:?}",
        labels
    );
    assert!(
        labels.iter().any(|label| label == "Build and validate"),
        "expected collapsible section header"
    );
    assert!(
        labels.iter().any(|label| label.contains("Overlays")),
        "expected overlay row label"
    );
}

#[test]
fn fields_window_body_has_populated_collapsible_content() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_fields_window_panel)
        .expect("setup_fields_window_panel");

    let mut world = app.world_mut();
    let bodies: Vec<_> = world
        .query::<&DevCollapsibleBody>()
        .iter(&mut world)
        .filter(|body| body.id == DevCollapsibleSectionId::FieldsBuild)
        .collect();
    assert_eq!(bodies.len(), 1);

    let button_count = world
        .query::<&DevTerrainFieldButton>()
        .iter(&mut world)
        .count();
    assert!(
        button_count >= 8,
        "expected full terrain field button row set, got {button_count}"
    );

    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<DevTerrainFieldButton>>()
        .iter(&mut world)
        .collect();
    for entity in entities {
        assert!(world.get::<Button>(entity).is_some());
    }
}

fn init_terrain_field_handler_resources(app: &mut App) {
    app.init_resource::<WorldConfig>()
        .init_resource::<WorldData>()
        .init_resource::<BuildingCatalog>()
        .init_resource::<FootprintCatalog>()
        .init_resource::<BuildingFieldRequirementCatalog>()
        .init_resource::<FieldResponseProfileCatalog>()
        .init_resource::<BuildingFieldRequirementCatalogRevision>()
        .init_resource::<FieldResponseProfileCatalogRevision>()
        .init_resource::<BuildingTerrainAssessmentStore>()
        .init_resource::<TerrainOverlayState>()
        .insert_resource(TerrainFieldCatalog::default())
        .insert_resource(TerrainFieldSourceProfileCatalog::default());
}

#[test]
fn fields_panel_visibility_tracks_window_open_state() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_fields_window_panel)
        .expect("setup_fields_window_panel");

    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        let mut registry = app.world_mut().resource_mut::<DevWindowRegistry>();
        registry.show(DevWindowId::Fields);
    }

    app.world_mut()
        .run_system_once(sync_dev_fields_panel_visibility)
        .expect("sync_dev_fields_panel_visibility");

    let mut world = app.world_mut();
    let open_vis = *world
        .query_filtered::<&Visibility, With<DevFieldsWindowUi>>()
        .single(&mut world)
        .expect("Fields panel visibility");
    assert!(
        matches!(open_vis, Visibility::Inherited | Visibility::Visible),
        "expected visible Fields panel, got {open_vis:?}"
    );

    {
        let mut registry = app.world_mut().resource_mut::<DevWindowRegistry>();
        registry.hide(DevWindowId::Fields);
    }

    app.world_mut()
        .run_system_once(sync_dev_fields_panel_visibility)
        .expect("sync_dev_fields_panel_visibility hidden");

    let mut world = app.world_mut();
    let hidden_vis = *world
        .query_filtered::<&Visibility, With<DevFieldsWindowUi>>()
        .single(&mut world)
        .expect("Fields panel visibility");
    assert_eq!(hidden_vis, Visibility::Hidden);
}

fn probe_button_entity(world: &mut World) -> Entity {
    world
        .query::<(Entity, &DevTerrainFieldButton)>()
        .iter(world)
        .find(|(_, button)| button.action == DevTerrainFieldAction::ToggleProbe)
        .map(|(entity, _)| entity)
        .expect("Probe DevTerrainFieldButton")
}

#[test]
fn pressed_probe_button_toggles_field_probe_state() {
    let mut app = headless_world_ui_app();
    init_terrain_field_handler_resources(&mut app);
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_fields_window_panel)
        .expect("setup_fields_window_panel");
    app.world_mut()
        .run_system_once(setup_dev_terrain_field_state)
        .expect("setup_dev_terrain_field_state");

    let probe = probe_button_entity(app.world_mut());
    app.world_mut()
        .entity_mut(probe)
        .insert(Interaction::Pressed);

    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        let mut registry = app.world_mut().resource_mut::<DevWindowRegistry>();
        registry.show(DevWindowId::Fields);
    }

    app.world_mut()
        .run_system_once(handle_terrain_field_buttons)
        .expect("handle_terrain_field_buttons");

    let field_state = app.world().resource::<DevTerrainFieldState>();
    assert!(
        !field_state.probe_enabled,
        "pressing Probe should toggle probe_enabled from default true"
    );

    app.world_mut()
        .run_system_once(sync_terrain_field_button_styles)
        .expect("sync_terrain_field_button_styles");
}
