//! World window panel construction and interaction tests.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use crate::dev::dev_mode::{DevModeInputGate, DevModeState};
use crate::dev::settlement_placement::{
    DevSettlementPlacementButton, DevUnitAssignmentButton, DevWorldSettlementSection,
    handle_settlement_placement_button, handle_unit_assignment_button,
};
use crate::dev::widgets::DevCollapsibleState;
use crate::dev::window::{DevWindowId, DevWindowRegistry, setup_dev_workspace};
use crate::dev::world_window::setup_world_window_panel;
use crate::units::input::SelectedUnits;
use crate::world::{
    Affiliation, ChunkCoord, ChunkData, ChunkLayout, Heightfield, InventoryCatalogCtx,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, LocalPosition, SettlementKind,
    SettlementOwnership, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WorldData,
    WorldPosition, assign_selected_units_at_position, create_settlement,
    create_unit_with_inventory, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
};

fn headless_world_ui_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::ui::UiPlugin));
    app.init_resource::<DevWindowRegistry>()
        .init_resource::<DevModeState>()
        .init_resource::<DevModeInputGate>()
        .init_resource::<DevCollapsibleState>();
    app
}

fn settlement_placement_button_entity(world: &mut World) -> Entity {
    world
        .query::<(Entity, &DevSettlementPlacementButton)>()
        .iter(world)
        .map(|(entity, _)| entity)
        .next()
        .expect("DevSettlementPlacementButton in World panel")
}

#[test]
fn world_panel_spawns_one_settlement_placement_button() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_world_window_panel)
        .expect("setup_world_window_panel");

    let mut world = app.world_mut();
    let buttons: Vec<_> = world
        .query::<&DevSettlementPlacementButton>()
        .iter(&mut world)
        .collect();
    assert_eq!(
        buttons.len(),
        1,
        "expected exactly one settlement placement control"
    );

    let sections: Vec<_> = world
        .query::<&DevWorldSettlementSection>()
        .iter(&mut world)
        .collect();
    assert_eq!(sections.len(), 1, "expected one Settlement section root");
}

#[test]
fn settlement_placement_button_is_graphical_with_label() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_world_window_panel)
        .expect("setup_world_window_panel");

    let entity = settlement_placement_button_entity(app.world_mut());
    let world = app.world();
    assert!(world.get::<Button>(entity).is_some());
    assert!(
        world
            .get::<crate::dev::widgets::DevWidgetActionButton>(entity)
            .is_some()
    );

    let mut world = app.world_mut();
    let labels: Vec<String> = world
        .query::<&Text>()
        .iter(&mut world)
        .map(|text| text.to_string())
        .collect();
    assert!(
        labels
            .iter()
            .any(|label| label == "Place Settlement Anchor"),
        "expected button label among spawned World panel strings"
    );
    assert!(
        labels.iter().any(|label| label == "Settlement"),
        "expected Settlement section header among spawned World panel strings"
    );
}

#[test]
fn pressed_settlement_placement_button_arms_placement() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_world_window_panel)
        .expect("setup_world_window_panel");

    let button = settlement_placement_button_entity(app.world_mut());
    app.world_mut()
        .entity_mut(button)
        .insert(Interaction::Pressed);

    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        let mut registry = app.world_mut().resource_mut::<DevWindowRegistry>();
        registry.show(DevWindowId::World);
    }

    app.world_mut()
        .run_system_once(handle_settlement_placement_button)
        .expect("handle_settlement_placement_button");

    let dev = app.world().resource::<DevModeState>();
    assert!(
        dev.settlement_placement_armed,
        "pressing constructed control should arm settlement placement"
    );
    assert!(
        dev.settlement_placement_message.contains("armed"),
        "expected armed status message, got `{}`",
        dev.settlement_placement_message
    );
}

fn unit_assignment_button_entity(world: &mut World) -> Entity {
    world
        .query::<(Entity, &DevUnitAssignmentButton)>()
        .iter(world)
        .map(|(entity, _)| entity)
        .next()
        .expect("DevUnitAssignmentButton in World panel")
}

#[test]
fn world_panel_spawns_unit_assignment_button() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_world_window_panel)
        .expect("setup_world_window_panel");

    let mut world = app.world_mut();
    let buttons: Vec<_> = world
        .query::<&DevUnitAssignmentButton>()
        .iter(&mut world)
        .collect();
    assert_eq!(buttons.len(), 1);
}

#[test]
fn pressed_unit_assignment_button_arms_assignment() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_world_window_panel)
        .expect("setup_world_window_panel");

    let button = unit_assignment_button_entity(app.world_mut());
    app.world_mut()
        .entity_mut(button)
        .insert(Interaction::Pressed);

    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        let mut registry = app.world_mut().resource_mut::<DevWindowRegistry>();
        registry.show(DevWindowId::World);
    }

    app.world_mut()
        .run_system_once(handle_unit_assignment_button)
        .expect("handle_unit_assignment_button");

    let dev = app.world().resource::<DevModeState>();
    assert!(dev.unit_assignment_armed);
    assert!(dev.unit_assignment_message.contains("armed"));
}

#[test]
fn armed_assignment_assigns_selected_units_to_clicked_settlement() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let profiles =
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap();
    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();

    let mut world_data = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
    world_data.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    let settlement = create_settlement(
        &mut world_data,
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(40.0, 0.0, 40.0)),
        ),
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
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(5.0, 0.0, 5.0)),
        ),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &ctx,
    )
    .unwrap();

    let (assigned_id, count) = assign_selected_units_at_position(
        &mut world_data,
        &[unit.id],
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(40.0, 0.0, 40.0)),
        ),
    )
    .unwrap();
    assert_eq!(assigned_id, settlement.settlement_id);
    assert_eq!(count, 1);
    assert_eq!(
        world_data.get_unit(unit.id).unwrap().settlement_id,
        Some(settlement.settlement_id)
    );
}
