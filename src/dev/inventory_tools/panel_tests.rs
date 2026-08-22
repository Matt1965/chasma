//! Dev Items panel interaction tests.

use bevy::prelude::*;

use crate::client::selection::WorldSelectionState;
use crate::dev::dev_mode::{DefinitionId, DevModeState, DevTab};
use crate::dev::inventory_tools::panel::{
    DevItemsAction, DevItemsButton, DevItemsSection, handle_dev_items_buttons,
    sync_items_section_visibility,
};
use crate::simulation::SimulationControlState;
use crate::ui::gameplay::GameplayBuildingSelection;
use crate::units::input::SelectedUnits;
use crate::world::{
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, ItemPileSettings,
    UnitCatalog, WorldConfig, WorldData,
};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<DevModeState>()
        .init_resource::<WorldConfig>()
        .init_resource::<WorldData>()
        .init_resource::<WorldSelectionState>()
        .init_resource::<GameplayBuildingSelection>()
        .init_resource::<SelectedUnits>()
        .init_resource::<UnitCatalog>()
        .init_resource::<ItemCatalog>()
        .init_resource::<ItemCategoryCatalog>()
        .init_resource::<InventoryProfileCatalog>()
        .init_resource::<ItemPileSettings>()
        .init_resource::<SimulationControlState>()
        .init_resource::<crate::dev::DevModeInputGate>();
    app
}

#[test]
fn items_section_uses_flex_display_when_items_tab_active() {
    let mut app = headless_app();
    let entity = app
        .world_mut()
        .spawn((
            DevItemsSection,
            Node {
                display: Display::None,
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();

    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        dev.active_tab = DevTab::Items;
    }

    app.add_systems(Update, sync_items_section_visibility);
    app.update();

    let node = app.world().get::<Node>(entity).unwrap();
    assert_eq!(node.display, Display::Flex);
    assert_eq!(
        *app.world().get::<Visibility>(entity).unwrap(),
        Visibility::Visible
    );
}

#[test]
fn pressed_add_to_unit_button_reaches_action_handler() {
    let mut app = headless_app();
    let button = app
        .world_mut()
        .spawn((
            DevItemsButton {
                action: DevItemsAction::AddToUnit,
            },
            Button,
            Interaction::Pressed,
        ))
        .id();

    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        dev.active_tab = DevTab::Items;
        dev.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));
        dev.inventory.quantity = 25;
    }

    app.add_systems(Update, handle_dev_items_buttons);
    app.update();

    let dev = app.world().resource::<DevModeState>();
    assert!(
        dev.inventory.message.contains("Added")
            || dev.inventory.message.contains("select")
            || dev.inventory.message.contains("unit")
            || dev.inventory.message.contains("No"),
        "expected handler to set inventory.message, got `{}`",
        dev.inventory.message
    );
    assert_eq!(
        *app.world().get::<Interaction>(button).unwrap(),
        Interaction::Pressed
    );
}

#[test]
fn pressed_spawn_pile_button_arms_placement() {
    let mut app = headless_app();
    app.world_mut().spawn((
        DevItemsButton {
            action: DevItemsAction::ArmPilePlacement,
        },
        Button,
        Interaction::Pressed,
    ));

    {
        let mut dev = app.world_mut().resource_mut::<DevModeState>();
        dev.enabled = true;
        dev.active_tab = DevTab::Items;
        dev.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));
    }

    app.add_systems(Update, handle_dev_items_buttons);
    app.update();

    assert!(
        app.world()
            .resource::<DevModeState>()
            .inventory
            .pile_placement_armed
    );
}
