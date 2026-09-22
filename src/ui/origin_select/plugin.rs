//! Continuous origin/squad stage plugin (CG8).

use bevy::prelude::*;

use crate::menu::AppScreen;
use crate::units::presentation::propagate_preview_render_layers;
use crate::ui::unit_editor::{
    cleanup_unit_editor_preview_studio, discover_preview_animation_players,
    ensure_preview_animation_playback, handle_unit_editor_sliders,
    install_preview_animation_graph, setup_unit_editor_preview_studio, sync_preview_idle_animation,
    sync_unit_editor_control_values, sync_unit_editor_error_text, update_unit_editor_preview_framing,
};
use crate::units::UnitAnimationSystems;

use super::actions::{
    cleanup_origin_select_session, handle_origin_squad_buttons, init_starting_squad_session_on_enter,
    respawn_origin_squad_ui_after_focus,
};
use super::camera::{rotate_origin_select_preview_roster, update_origin_select_preview_camera};
use super::focus::{
    despawn_origin_squad_focus_ui, handle_origin_squad_focus_done, sync_origin_squad_focus_ui,
};
use super::presentation::sync_origin_select_preview_presentation;
use super::preview::{
    cleanup_stray_unit_editor_preview_actors, sync_origin_select_preview_roster,
};
use super::screen::{
    despawn_origin_select_ui, spawn_origin_select_preview_ui, spawn_origin_select_squad_panel,
    sync_origin_select_preview_viewport, sync_origin_squad_origin_text,
};

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct OriginSelectSystems;

pub struct OriginSelectPlugin;

fn origin_squad_focused(session: Option<Res<crate::menu::StartingSquadSession>>) -> bool {
    session.is_some_and(|value| value.is_focused())
}

impl Plugin for OriginSelectPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, OriginSelectSystems)
            .add_systems(
                OnEnter(AppScreen::OriginSelect),
                (
                    setup_unit_editor_preview_studio,
                    init_starting_squad_session_on_enter,
                    spawn_origin_select_preview_ui,
                    spawn_origin_select_squad_panel,
                    cleanup_stray_unit_editor_preview_actors,
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
            .add_systems(
                Update,
                (
                    handle_origin_squad_buttons,
                    handle_origin_squad_focus_done,
                    cleanup_stray_unit_editor_preview_actors,
                    sync_origin_squad_focus_ui,
                    respawn_origin_squad_ui_after_focus,
                    sync_origin_squad_origin_text,
                    sync_origin_select_preview_viewport,
                    sync_origin_select_preview_roster,
                    update_unit_editor_preview_framing,
                    propagate_preview_render_layers,
                    discover_preview_animation_players,
                    install_preview_animation_graph,
                    sync_preview_idle_animation,
                    ensure_preview_animation_playback.after(UnitAnimationSystems),
                    sync_origin_select_preview_presentation,
                    update_origin_select_preview_camera,
                    rotate_origin_select_preview_roster,
                )
                    .run_if(in_state(AppScreen::OriginSelect))
                    .in_set(OriginSelectSystems),
            )
            .add_systems(
                Update,
                (
                    handle_unit_editor_sliders,
                    sync_unit_editor_control_values,
                    sync_unit_editor_error_text,
                )
                    .run_if(in_state(AppScreen::OriginSelect))
                    .run_if(origin_squad_focused)
                    .in_set(OriginSelectSystems),
            );
    }
}
