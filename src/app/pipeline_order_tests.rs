//! Frame-order contract for the `Update` schedule.
//!
//! Ordering cycles are rejected while `Schedule::initialize` builds the graph,
//! so they abort the app on its first frame rather than failing a system test.
//! Declaring the real set relationships against placeholder systems reproduces
//! that failure without the graphical app or any gameplay resource.

use bevy::ecs::schedule::Schedules;
use bevy::prelude::*;

use super::{ViewFocusSystems, configure_update_pipeline_sets};
use crate::buildings::BuildingRuntimeSystems;
use crate::camera::CameraControlSystems;
use crate::client::{
    ClientIntentCollectSystems, ClientIntentDispatchSystems, ClientIntentFlushSystems,
};
use crate::doodads::DoodadRuntimeSystems;
use crate::item_piles::ItemPileRuntimeSystems;
use crate::menu::MenuInputSystems;
use crate::player::configure_player_control_sets;
use crate::player::{
    DebugPresentationSystems, GameplayPresentationSystems, PlayerControlSystems, RuntimeSyncSystems,
};
use crate::projectiles::ProjectileRuntimeSystems;
use crate::simulation::{SimulationControlSystems, SimulationSystems};
use crate::terrain::TerrainStreamingSystems;
use crate::ui::gameplay::{GameplayCommandInputSystems, GameplayInputGateSystems};
use crate::units::{UnitAnimationSystems, UnitRuntimeSystems};

fn noop() {}

/// Declare the real ordering contract, with one placeholder system per set.
///
/// Cycles only appear once sets own systems: Bevy flattens set ordering onto
/// members, so an empty set can never close a loop.
fn pipeline_app() -> App {
    let mut app = App::new();
    configure_update_pipeline_sets(&mut app);
    configure_player_control_sets(&mut app);

    app.add_systems(
        Update,
        (
            noop.in_set(MenuInputSystems),
            noop.in_set(CameraControlSystems),
            noop.in_set(ViewFocusSystems),
            noop.in_set(TerrainStreamingSystems),
            noop.in_set(DoodadRuntimeSystems),
            noop.in_set(ItemPileRuntimeSystems),
            noop.in_set(BuildingRuntimeSystems),
            noop.in_set(UnitRuntimeSystems),
            noop.in_set(ProjectileRuntimeSystems),
            noop.in_set(UnitAnimationSystems),
        ),
    )
    .add_systems(
        Update,
        (
            noop.in_set(SimulationControlSystems),
            noop.in_set(SimulationSystems),
            noop.in_set(GameplayInputGateSystems),
            noop.in_set(ClientIntentCollectSystems),
            noop.in_set(GameplayCommandInputSystems),
            noop.in_set(ClientIntentDispatchSystems),
            noop.in_set(ClientIntentFlushSystems),
            noop.in_set(GameplayPresentationSystems),
            noop.in_set(DebugPresentationSystems),
        ),
    );
    #[cfg(feature = "dev")]
    app.add_systems(
        Update,
        (
            noop.in_set(crate::dev::DevModeInputSystems),
            noop.in_set(crate::dev::DevModePresentationSystems),
        ),
    );
    app
}

/// Build the `Update` graph, returning the rendered error on failure.
fn build_update_schedule(app: &mut App) -> Result<(), String> {
    let world = app.world_mut();
    // Taking the resource (rather than `resource_scope`) keeps schedule
    // construction free to insert resources of its own.
    let mut schedules = world
        .remove_resource::<Schedules>()
        .expect("app has schedules");
    let schedule = schedules.get_mut(Update).expect("Update schedule exists");
    let result = schedule.initialize(world).map_err(|err| format!("{err}"));
    world.insert_resource(schedules);
    result
}

#[test]
fn update_pipeline_order_has_no_cycles() {
    let mut app = pipeline_app();
    if let Err(err) = build_update_schedule(&mut app) {
        panic!("Update frame order is unsatisfiable: {err}");
    }
}

#[test]
fn player_control_cannot_be_ordered_before_camera() {
    // `PlayerControlSystems` runs after `RuntimeSyncSystems`, which runs after
    // camera via view focus and terrain streaming. Camera therefore reads HUD
    // state produced by the previous frame, and pulling any player-control set
    // ahead of camera closes that loop.
    let mut app = pipeline_app();
    app.configure_sets(
        Update,
        GameplayInputGateSystems.before(CameraControlSystems),
    );

    assert!(
        build_update_schedule(&mut app).is_err(),
        "ordering a player-control set before camera must be rejected as a cycle"
    );
}
