//! World window panel construction tests (settlement controls relocated).

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use crate::dev::dev_mode::{DevModeInputGate, DevModeState};
use crate::dev::settlement_placement::DevSettlementPlacementButton;
use crate::dev::settlement_window::panel::DevSettlementAddUnitsButton;
use crate::dev::widgets::DevCollapsibleState;
use crate::dev::window::setup_dev_workspace;
use crate::dev::world_window::setup_world_window_panel;

fn headless_world_ui_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::ui::UiPlugin));
    app.init_resource::<DevWindowRegistry>()
        .init_resource::<DevModeState>()
        .init_resource::<DevModeInputGate>()
        .init_resource::<DevCollapsibleState>();
    app
}

use crate::dev::window::DevWindowRegistry;

#[test]
fn world_panel_no_longer_spawns_settlement_controls() {
    let mut app = headless_world_ui_app();
    app.world_mut()
        .run_system_once(setup_dev_workspace)
        .expect("setup_dev_workspace");
    app.world_mut()
        .run_system_once(setup_world_window_panel)
        .expect("setup_world_window_panel");

    let mut world = app.world_mut();
    assert_eq!(
        world
            .query::<&DevSettlementPlacementButton>()
            .iter(&mut world)
            .count(),
        0
    );
    assert_eq!(
        world
            .query::<&DevSettlementAddUnitsButton>()
            .iter(&mut world)
            .count(),
        0
    );
}
