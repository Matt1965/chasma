use bevy::prelude::*;
use chasma::app::AppPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(AppPlugin)
        .run();
}
