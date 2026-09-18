//! Continuous origin/squad stage plugin (CG8).

use bevy::prelude::*;

use crate::menu::AppScreen;
use crate::units::presentation::propagate_preview_render_layers;
use crate::ui::unit_editor::{
    cleanup_unit_editor_preview_studio, handle_unit_editor_sliders, setup_unit_editor_preview_studio,
    sync_unit_editor_control_values, sync_unit_editor_error_text,
    update_unit_editor_preview_framing,
};

use super::actions::{
    cleanup_origin_select_session, handle_origin_squad_buttons, init_starting_squad_session_on_enter,
    respawn_origin_squad_ui_after_focus,
};
use super::camera::{rotate_origin_select_preview_roster, update_origin_select_preview_camera};
use super::focus::{
    despawn_origin_squad_focus_ui, handle_origin_squad_focus_done, sync_origin_squad_focus_ui,
};
use super::preview::sync_origin_select_preview_roster;
use super::screen::{
    despawn_origin_select_ui, spawn_origin_select_ui, sync_origin_squad_origin_text,
};

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct OriginSelectSystems;

pub struct OriginSelectPlugin;

fn origin_squad_focused(session: Res<crate::menu::StartingSquadSession>) -> bool {
    session.is_focused()
}

impl Plugin for OriginSelectPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, OriginSelectSystems)
            .add_systems(
                OnEnter(AppScreen::OriginSelect),
                (
                    setup_unit_editor_preview_studio,
                    init_starting_squad_session_on_enter,
                    spawn_origin_select_ui,
                )
                    .chain(),
            )
            .add_systems(
                OnExit(AppScreen::OriginSelect),
                (
                    despawn_origin_select_ui,
                    despawn_origin_squad_focus_ui,
                    cleanup_unit_editor_preview_studio,
                    cleanup_origin_select_session,
                )
                    .chain(),
            )
            .add_systems(Update, handle_origin_squad_buttons)
            .add_systems(Update, handle_origin_squad_focus_done)
            .add_systems(Update, sync_origin_squad_focus_ui)
            .add_systems(Update, respawn_origin_squad_ui_after_focus)
            .add_systems(
                Update,
                (
                    handle_unit_editor_sliders,
                    sync_unit_editor_control_values,
                    sync_unit_editor_error_text,
                )
                    .run_if(origin_squad_focused)
                    .in_set(OriginSelectSystems),
            )
            .add_systems(Update, sync_origin_squad_origin_text.in_set(OriginSelectSystems))
            .add_systems(Update, sync_origin_select_preview_roster.in_set(OriginSelectSystems))
            .add_systems(Update, update_unit_editor_preview_framing.in_set(OriginSelectSystems))
            .add_systems(Update, propagate_preview_render_layers.in_set(OriginSelectSystems))
            .add_systems(Update, update_origin_select_preview_camera.in_set(OriginSelectSystems))
            .add_systems(Update, rotate_origin_select_preview_roster.in_set(OriginSelectSystems));
    }
}
