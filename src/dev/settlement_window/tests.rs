//! Settlement Dev window tests.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use crate::client::CameraSettlementContext;
use crate::dev::dev_mode::{DevModeInputGate, DevModeState};
use crate::dev::settlement_placement::{
    DevSettlementPlacementButton, handle_settlement_placement_button, place_settlement_anchor,
};
use crate::dev::settlement_window::panel::{DevSettlementAddUnitsButton, DevSettlementAiToggle};
use crate::dev::settlement_window::{
    build_settlement_dev_summary, format_focused_line, handle_settlement_add_units_button,
    handle_settlement_ai_toggle, setup_settlement_window_panel,
    sync_settlement_dev_action_availability, sync_settlement_dev_panel,
};
use crate::dev::widgets::{DevCollapsibleState, DevWidgetActionButton};
use crate::dev::window::{DevWindowId, DevWindowRegistry, setup_dev_workspace};
use crate::dev::world_window::setup_world_window_panel;
use crate::simulation::SimulationControlState;
use crate::units::input::SelectedUnits;
use crate::world::{
    Affiliation, ChunkCoord, ChunkData, ChunkLayout, Heightfield, InventoryCatalogCtx,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, LocalPosition, SettlementKind,
    SettlementOwnership, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WorldConfig,
    WorldData, WorldPosition, create_settlement, create_unit_with_inventory,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_unit_definitions,
};

fn headless_ui_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::ui::UiPlugin));
    app.init_resource::<DevWindowRegistry>()
        .init_resource::<DevModeState>()
        .init_resource::<DevModeInputGate>()
        .init_resource::<DevCollapsibleState>()
        .init_resource::<WorldConfig>()
        .init_resource::<WorldData>()
        .init_resource::<CameraSettlementContext>()
        .init_resource::<SelectedUnits>()
        .init_resource::<SimulationControlState>();
    app
}

fn setup_settlement_panel(app: &mut App) {
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("workspace");
    app.world_mut()
        .run_system_once(setup_settlement_window_panel)
        .expect("settlement panel");
}

fn test_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

#[test]
fn settlement_window_spawns_controls_not_world_panel() {
    let mut app = headless_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("workspace");
    app.world_mut()
        .run_system_once(setup_world_window_panel)
        .expect("world panel");
    app.world_mut()
        .run_system_once(setup_settlement_window_panel)
        .expect("settlement panel");

    let mut world = app.world_mut();
    assert_eq!(
        world
            .query::<&DevSettlementPlacementButton>()
            .iter(&mut world)
            .count(),
        1
    );
    assert_eq!(
        world
            .query::<&DevSettlementAddUnitsButton>()
            .iter(&mut world)
            .count(),
        1
    );
}

#[test]
fn panel_summary_uses_camera_settlement_context() {
    let mut app = headless_ui_app();
    setup_settlement_panel(&mut app);
    let mut world_data = test_world();
    let report = create_settlement(
        &mut world_data,
        pos(64.0, 64.0),
        "New Haven",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        Some(48.0),
        None,
        0,
    )
    .unwrap();
    *app.world_mut().resource_mut::<WorldData>() = world_data;
    *app.world_mut().resource_mut::<CameraSettlementContext>() = CameraSettlementContext {
        focused_settlement_id: Some(report.settlement_id),
        focus_world_position: Some(pos(64.0, 64.0)),
    };
    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        app.world_mut()
            .resource_mut::<DevWindowRegistry>()
            .show(DevWindowId::Settlement);
    }
    app.world_mut()
        .run_system_once(sync_settlement_dev_panel)
        .expect("sync");

    let mut world = app.world_mut();
    let labels: Vec<String> = world
        .query::<&Text>()
        .iter(&mut world)
        .map(|text| text.to_string())
        .collect();
    assert!(labels.iter().any(|label| label == "Focused: New Haven"));
}

#[test]
fn none_context_shows_no_focused_settlement() {
    let mut app = headless_ui_app();
    setup_settlement_panel(&mut app);
    *app.world_mut().resource_mut::<WorldData>() = test_world();
    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        app.world_mut()
            .resource_mut::<DevWindowRegistry>()
            .show(DevWindowId::Settlement);
    }
    app.world_mut()
        .run_system_once(sync_settlement_dev_panel)
        .expect("sync");
    let summary = build_settlement_dev_summary(
        app.world().resource::<WorldData>(),
        app.world().resource::<CameraSettlementContext>(),
    );
    assert_eq!(
        format_focused_line(&summary),
        "Focused: No focused settlement"
    );
}

#[test]
fn create_settlement_uses_existing_placement_authority() {
    let mut world = test_world();
    let before = world.settlement_store().sorted_settlement_ids().len();
    place_settlement_anchor(
        &mut world,
        pos(30.0, 30.0),
        SettlementOwnership::player_default(),
        0,
    )
    .expect("placement");
    assert_eq!(
        world.settlement_store().sorted_settlement_ids().len(),
        before + 1
    );
}

#[test]
fn add_selected_units_targets_focused_settlement() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let profiles =
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap();
    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();

    let mut app = headless_ui_app();
    setup_settlement_panel(&mut app);
    let mut world_data = test_world();
    let settlement = create_settlement(
        &mut world_data,
        pos(40.0, 40.0),
        "Target",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap();
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &mut world_data,
        &UnitDefinitionId::new("bandit"),
        pos(5.0, 5.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &ctx,
    )
    .unwrap();
    *app.world_mut().resource_mut::<WorldData>() = world_data;
    *app.world_mut().resource_mut::<CameraSettlementContext>() = CameraSettlementContext {
        focused_settlement_id: Some(settlement.settlement_id),
        focus_world_position: None,
    };
    app.world_mut()
        .resource_mut::<SelectedUnits>()
        .0
        .insert(unit.id);
    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        app.world_mut()
            .resource_mut::<DevWindowRegistry>()
            .show(DevWindowId::Settlement);
    }

    let button = app
        .world_mut()
        .query::<(Entity, &DevSettlementAddUnitsButton)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("add units button");
    app.world_mut()
        .entity_mut(button)
        .insert(Interaction::Pressed);
    app.world_mut()
        .run_system_once(handle_settlement_add_units_button)
        .expect("handler");

    let world = app.world().resource::<WorldData>();
    assert_eq!(
        world.get_unit(unit.id).unwrap().settlement_id,
        Some(settlement.settlement_id)
    );
}

#[test]
fn no_selected_units_disables_add_button() {
    let mut app = headless_ui_app();
    setup_settlement_panel(&mut app);
    let mut world_data = test_world();
    let settlement = create_settlement(
        &mut world_data,
        pos(40.0, 40.0),
        "Target",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap();
    *app.world_mut().resource_mut::<WorldData>() = world_data;
    *app.world_mut().resource_mut::<CameraSettlementContext>() = CameraSettlementContext {
        focused_settlement_id: Some(settlement.settlement_id),
        focus_world_position: None,
    };
    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        app.world_mut()
            .resource_mut::<DevWindowRegistry>()
            .show(DevWindowId::Settlement);
    }
    app.world_mut()
        .run_system_once(sync_settlement_dev_action_availability)
        .expect("sync availability");
    let mut world = app.world_mut();
    let disabled = world
        .query::<(&DevWidgetActionButton, &DevSettlementAddUnitsButton)>()
        .iter(&mut world)
        .map(|(button, _)| button.disabled)
        .next()
        .expect("add units button");
    assert!(disabled);
}

#[test]
fn no_focused_settlement_disables_targeted_actions() {
    let mut app = headless_ui_app();
    setup_settlement_panel(&mut app);
    *app.world_mut().resource_mut::<WorldData>() = test_world();
    app.world_mut()
        .resource_mut::<SelectedUnits>()
        .0
        .insert(crate::world::UnitId::new(1));
    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        app.world_mut()
            .resource_mut::<DevWindowRegistry>()
            .show(DevWindowId::Settlement);
    }
    app.world_mut()
        .run_system_once(sync_settlement_dev_action_availability)
        .expect("sync availability");
    let mut world = app.world_mut();
    let disabled = world
        .query::<(&DevWidgetActionButton, &DevSettlementAddUnitsButton)>()
        .iter(&mut world)
        .map(|(button, _)| button.disabled)
        .next()
        .expect("add units button");
    assert!(disabled);
}

#[test]
fn ai_toggle_mutates_automation_enabled_policy() {
    let mut app = headless_ui_app();
    setup_settlement_panel(&mut app);
    let mut world_data = test_world();
    let settlement = create_settlement(
        &mut world_data,
        pos(40.0, 40.0),
        "Target",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap();
    world_data
        .settlement_state_store_mut()
        .get_mut(settlement.settlement_id)
        .expect("state")
        .policies
        .automation_enabled = true;
    *app.world_mut().resource_mut::<WorldData>() = world_data;
    *app.world_mut().resource_mut::<CameraSettlementContext>() = CameraSettlementContext {
        focused_settlement_id: Some(settlement.settlement_id),
        focus_world_position: None,
    };
    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        app.world_mut()
            .resource_mut::<DevWindowRegistry>()
            .show(DevWindowId::Settlement);
    }
    let toggle = app
        .world_mut()
        .query::<(Entity, &DevSettlementAiToggle)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("ai toggle");
    app.world_mut()
        .entity_mut(toggle)
        .insert(Interaction::Pressed);
    app.world_mut()
        .run_system_once(handle_settlement_ai_toggle)
        .expect("toggle");
    let world = app.world().resource::<WorldData>();
    let enabled = world
        .settlement_state_store()
        .get(settlement.settlement_id)
        .expect("state")
        .policies
        .automation_enabled;
    assert!(!enabled);
}

#[test]
fn placement_button_arms_from_settlement_window() {
    let mut app = headless_ui_app();
    setup_settlement_panel(&mut app);
    let button = app
        .world_mut()
        .query::<(Entity, &DevSettlementPlacementButton)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("placement button");
    app.world_mut()
        .entity_mut(button)
        .insert(Interaction::Pressed);
    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        app.world_mut()
            .resource_mut::<DevWindowRegistry>()
            .show(DevWindowId::Settlement);
    }
    app.world_mut()
        .run_system_once(handle_settlement_placement_button)
        .expect("handler");
    assert!(
        app.world()
            .resource::<DevModeState>()
            .settlement_placement_armed
    );
}
