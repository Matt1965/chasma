//! Origin selection plugin (CG7).

use bevy::prelude::*;

use crate::menu::AppScreen;
use crate::units::presentation::propagate_preview_render_layers;
use crate::ui::unit_editor::{
    cleanup_unit_editor_preview_studio, setup_unit_editor_preview_studio,
};

use super::actions::{cleanup_origin_select_session, handle_origin_select_buttons};
use super::camera::{rotate_origin_select_preview_roster, update_origin_select_preview_camera};
use super::preview::sync_origin_select_preview_roster;
use super::screen::{despawn_origin_select_ui, spawn_origin_select_ui, sync_origin_select_list_highlight};
use super::session::OriginSelectSession;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct OriginSelectSystems;

pub struct OriginSelectPlugin;

impl Plugin for OriginSelectPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, OriginSelectSystems)
            .add_systems(
                OnEnter(AppScreen::OriginSelect),
                (
                    setup_unit_editor_preview_studio,
                    |mut commands: Commands| {
                        commands.init_resource::<OriginSelectSession>();
                    },
                    spawn_origin_select_ui,
                )
                    .chain(),
            )
            .add_systems(
                OnExit(AppScreen::OriginSelect),
                (
                    despawn_origin_select_ui,
                    cleanup_unit_editor_preview_studio,
                    cleanup_origin_select_session,
                )
                    .chain(),
            )
            .add_systems(Update, handle_origin_select_buttons)
            .add_systems(Update, sync_origin_select_list_highlight.in_set(OriginSelectSystems))
            .add_systems(Update, sync_origin_select_preview_roster.in_set(OriginSelectSystems))
            .add_systems(Update, propagate_preview_render_layers.in_set(OriginSelectSystems))
            .add_systems(Update, update_origin_select_preview_camera.in_set(OriginSelectSystems))
            .add_systems(Update, rotate_origin_select_preview_roster.in_set(OriginSelectSystems));
    }
}
