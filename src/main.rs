use bevy::prelude::*;
use bevy::window::WindowPlugin;
use chasma::app::AppPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Chasma".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(AppPlugin)
        .run();
}
