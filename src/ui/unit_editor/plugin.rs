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
    discover_preview_animation_players, install_preview_animation_graph, sync_preview_idle_animation,
};
use super::preview_studio::{
    cleanup_unit_editor_preview_studio, rotate_unit_editor_preview_unit,
    setup_unit_editor_preview_studio, update_unit_editor_preview_camera,
    update_unit_editor_preview_framing,
};
use super::screen::{
    despawn_unit_editor_ui, spawn_unit_editor_ui, sync_unit_editor_error_text,
};
use super::session::UnitEditorSession;

/// Unit Editor systems (screen-local presentation and input).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct UnitEditorSystems;

pub struct UnitEditorPlugin;

impl Plugin for UnitEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnitEditorSliderDragState>()
            .add_message::<super::events::OpenUnitEditorRequest>()
            .configure_sets(Update, UnitEditorSystems)
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
            .add_systems(Update, handle_unit_editor_escape)
            .add_systems(Update, handle_unit_editor_buttons)
            .add_systems(Update, handle_unit_editor_sliders.in_set(UnitEditorSystems))
            .add_systems(Update, sync_unit_editor_control_values.in_set(UnitEditorSystems))
            .add_systems(Update, sync_unit_editor_error_text.in_set(UnitEditorSystems))
            .add_systems(Update, sync_unit_editor_preview.in_set(UnitEditorSystems))
            .add_systems(Update, update_unit_editor_preview_framing.in_set(UnitEditorSystems))
            .add_systems(Update, propagate_preview_render_layers.in_set(UnitEditorSystems))
            .add_systems(Update, discover_preview_animation_players.in_set(UnitEditorSystems))
            .add_systems(Update, install_preview_animation_graph.in_set(UnitEditorSystems))
            .add_systems(Update, sync_preview_idle_animation.in_set(UnitEditorSystems))
            .add_systems(Update, rotate_unit_editor_preview_unit.in_set(UnitEditorSystems))
            .add_systems(Update, update_unit_editor_preview_camera.in_set(UnitEditorSystems));
    }
}
