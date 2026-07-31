//! Smoke tests that UI plugins schedule without Bevy B0001 query conflicts.
//!
//! B0001 is raised while a system's parameters are initialized against the
//! world, before the system ever executes. Building and initializing every
//! schedule therefore exercises the exact failure mode without requiring the
//! full graphical application or every gameplay resource.

use bevy::ecs::schedule::Schedules;
use bevy::prelude::*;
use bevy::ui::UiPlugin;

use super::DevModePlugin;
use crate::ui::gameplay::GameplayUiPlugin;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), UiPlugin));
    app
}

/// Initialize every schedule, panicking on any conflicting system parameters.
///
/// Returns the number of systems that were initialized so callers can assert
/// the plugin under test actually contributed work.
fn initialize_all_schedules(app: &mut App) -> usize {
    app.finish();
    app.cleanup();

    let world = app.world_mut();
    // Taking the resource (rather than `resource_scope`) keeps schedule
    // initialization free to insert resources of its own.
    let mut schedules = world
        .remove_resource::<Schedules>()
        .expect("app has schedules");

    let mut initialized = 0usize;
    for (label, schedule) in schedules.iter_mut() {
        schedule
            .initialize(world)
            .unwrap_or_else(|err| panic!("schedule {label:?} failed to build: {err}"));
        initialized += schedule.systems_len();
    }

    world.insert_resource(schedules);
    initialized
}

#[test]
fn dev_ui_schedules_initialize_without_query_conflicts() {
    let mut app = headless_app();
    app.add_plugins(DevModePlugin);

    let baseline = initialize_all_schedules(&mut headless_app());
    let with_dev = initialize_all_schedules(&mut app);

    assert!(
        with_dev > baseline,
        "expected DevModePlugin to add systems (baseline {baseline}, with dev {with_dev})"
    );
}

#[test]
fn gameplay_ui_schedules_initialize_without_query_conflicts() {
    let mut app = headless_app();
    app.add_plugins(GameplayUiPlugin);

    let baseline = initialize_all_schedules(&mut headless_app());
    let with_ui = initialize_all_schedules(&mut app);

    assert!(
        with_ui > baseline,
        "expected GameplayUiPlugin to add systems (baseline {baseline}, with ui {with_ui})"
    );
}

/// Dev UI and gameplay UI share `Interaction`/`Text`/`Node` archetypes, so the
/// combination is the case most likely to surface an overlapping-query defect.
#[test]
fn dev_and_gameplay_ui_together_initialize_without_query_conflicts() {
    let mut app = headless_app();
    app.add_plugins((DevModePlugin, GameplayUiPlugin));
    initialize_all_schedules(&mut app);
}

/// Spawning entities that carry every broad UI marker at once forces Bevy to
/// resolve query disjointness against a real archetype rather than an empty
/// world.
#[test]
fn overlapping_ui_archetypes_do_not_conflict() {
    let mut app = headless_app();
    app.add_plugins((DevModePlugin, GameplayUiPlugin));

    app.world_mut().spawn((
        Node::default(),
        Button,
        Interaction::None,
        BackgroundColor(Color::WHITE),
        BorderColor::all(Color::BLACK),
        Visibility::Visible,
        Transform::default(),
        crate::dev::input::DevPanelUi,
        crate::dev::window::DevWindowUi,
    ));
    app.world_mut().spawn((
        Node::default(),
        Text::new("shared"),
        TextFont::default(),
        TextColor(Color::WHITE),
        Visibility::Visible,
        crate::dev::input::DevPanelUi,
        crate::dev::window::DevWindowUi,
    ));

    initialize_all_schedules(&mut app);
}
