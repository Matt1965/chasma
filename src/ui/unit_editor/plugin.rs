//! Unit Editor plugin registration (CG3).

use bevy::prelude::*;

use crate::menu::AppScreen;
use crate::units::presentation::propagate_preview_render_layers;

use super::preview_spawn::sync_unit_editor_preview;

use super::actions::{
    cleanup_unit_editor_session, handle_unit_editor_buttons, handle_unit_editor_escape,
};
use super::events::consume_open_unit_editor_requests;
use super::controls::{
    UnitEditorSliderDragState, handle_unit_editor_sliders, sync_unit_editor_control_values,
};
use super::preview_animation::{
    discover_preview_animation_players, ensure_preview_animation_playback,
    install_preview_animation_graph, sync_preview_idle_animation,
};
use crate::units::UnitAnimationSystems;
use super::preview_studio::{
    cleanup_unit_editor_preview_studio, rotate_unit_editor_preview_unit,
    setup_unit_editor_preview_studio, update_unit_editor_preview_camera,
    update_unit_editor_preview_framing,
};
use super::screen::{
    despawn_unit_editor_ui, spawn_unit_editor_ui, sync_unit_editor_error_text,
};

/// Unit Editor systems (screen-local presentation and input).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct UnitEditorSystems;

pub struct UnitEditorPlugin;

impl Plugin for UnitEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnitEditorSliderDragState>()
            .add_message::<super::events::OpenUnitEditorRequest>()
            .configure_sets(
                Update,
                UnitEditorSystems.run_if(in_state(AppScreen::UnitEditor)),
            )
            .add_systems(
                OnEnter(AppScreen::UnitEditor),
                (setup_unit_editor_preview_studio, spawn_unit_editor_ui).chain(),
            )
            .add_systems(
                OnExit(AppScreen::UnitEditor),
                (
                    despawn_unit_editor_ui,
                    cleanup_unit_editor_preview_studio,
                    cleanup_unit_editor_session,
                )
                    .chain(),
            )
            .add_systems(Update, consume_open_unit_editor_requests)
            .add_systems(
                Update,
                (
                    handle_unit_editor_escape,
                    handle_unit_editor_buttons,
                    handle_unit_editor_sliders,
                    sync_unit_editor_control_values,
                    sync_unit_editor_error_text,
                    sync_unit_editor_preview,
                    update_unit_editor_preview_framing,
                    propagate_preview_render_layers,
                    discover_preview_animation_players,
                    install_preview_animation_graph,
                    sync_preview_idle_animation,
                    ensure_preview_animation_playback.after(UnitAnimationSystems),
                    rotate_unit_editor_preview_unit,
                    update_unit_editor_preview_camera,
                )
                    .in_set(UnitEditorSystems),
            );
    }
}
