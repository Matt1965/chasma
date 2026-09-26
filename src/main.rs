use bevy::prelude::*;
use bevy::window::WindowPlugin;
use chasma::app::AppPlugin;
use chasma::logging::configure_log_plugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(configure_log_plugin())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Chasma".into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(AppPlugin)
        .run();
}
