//! Input → intent collection and client pipeline wiring (ADR-038 U-UI2).

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::{gameplay_input_blocked_by_hud, PlayerHudHoverState};
use crate::units::input::{
    cursor_screen_position, cursor_world_ray, normalized_screen_rect, pick_unit_along_ray,
    pick_unit_command_target_along_ray, terrain_click_to_world_position, BoxSelectDrag,
};
use crate::units::UnitRenderEntity;
use crate::world::{UnitCatalog, WorldConfig, WorldData};

use super::intent::{ClientInputModifiers, ClientIntent, ClientIntentQueue};

/// Client intent pipeline systems (collect → dispatch).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ClientPipelineSystems;

/// Collect phase — device input only; emits [`ClientIntent`] values.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ClientIntentCollectSystems;

/// Dispatch phase — routes intents to selection and command APIs.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ClientIntentDispatchSystems;

/// Registers client intent resources and pipeline systems.
pub struct ClientPipelinePlugin;

impl Plugin for ClientPipelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientIntentQueue>()
            .init_resource::<ClientInputModifiers>()
            .init_resource::<crate::client::commands::ResolvedCommandFeedback>();
    }
}

/// Sample modifiers and translate mouse input into intents (no selection/command side effects).
pub fn collect_unit_input_intents(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    unit_catalog: Res<UnitCatalog>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    units: Query<(&UnitRenderEntity, &GlobalTransform)>,
    mut queue: ResMut<ClientIntentQueue>,
    mut modifiers: ResMut<ClientInputModifiers>,
    mut box_drag: ResMut<BoxSelectDrag>,
    mut boundary: ResMut<crate::debug::ClientBoundaryGuard>,
    hud_hover: Res<PlayerHudHoverState>,
    #[cfg(feature = "dev")] gate: Res<crate::dev::DevModeInputGate>,
) {
    boundary.begin_input_collection();
    let shift = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if modifiers.shift != shift {
        queue.push(ClientIntent::ShiftModifier { pressed: shift });
    }
    modifiers.shift = shift;

    #[cfg(feature = "dev")]
    if crate::dev::DevModeInputGate::should_block(&gate) {
        boundary.end_input_collection();
        return;
    }

    if gameplay_input_blocked_by_hud(&hud_hover) {
        boundary.end_input_collection();
        return;
    }

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some(screen) = cursor_screen_position(&windows) {
            box_drag.begin(screen);
        }
    }

    if mouse_buttons.pressed(MouseButton::Left) {
        if let Some(screen) = cursor_screen_position(&windows) {
            box_drag.update(screen);
        }
    }

    let selection_policy = modifiers.selection_policy;

    if mouse_buttons.just_released(MouseButton::Left) {
        if box_drag.active {
            if box_drag.is_box_drag() {
                let (rect_min, rect_max) =
                    normalized_screen_rect(box_drag.start, box_drag.current);
                if shift {
                    queue.push(ClientIntent::BoxSelectAdd { rect_min, rect_max });
                } else {
                    queue.push(ClientIntent::BoxSelect { rect_min, rect_max });
                }
            } else if let Some(ray) = cursor_world_ray(&windows, &camera) {
                if let Some(unit_id) =
                    pick_unit_along_ray(&ray, &world, &unit_catalog, &units, selection_policy)
                {
                    if shift {
                        queue.push(ClientIntent::ToggleUnitSelection { unit_id });
                    } else {
                        queue.push(ClientIntent::SelectUnit { unit_id });
                    }
                } else if terrain_click_to_world_position(&ray, &world, layout, vertical_scale)
                    .is_some()
                    && !shift
                {
                    queue.push(ClientIntent::ClearSelection);
                }
            }
        }
        box_drag.reset();
    }

    if mouse_buttons.just_pressed(MouseButton::Right) {
        let Some(ray) = cursor_world_ray(&windows, &camera) else {
            boundary.end_input_collection();
            return;
        };

        use crate::client::CommandTarget;

        if let Some(unit_id) =
            pick_unit_command_target_along_ray(&ray, &world, &unit_catalog, &units)
        {
            queue.push(ClientIntent::ContextualCommand {
                target: CommandTarget::Unit { unit_id },
            });
        } else if let Some(click) =
            terrain_click_to_world_position(&ray, &world, layout, vertical_scale)
        {
            queue.push(ClientIntent::ContextualCommand {
                target: CommandTarget::Terrain {
                    position: click.world_position,
                },
            });
        }
    }

    boundary.end_input_collection();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::intent::{ClientIntent, ClientIntentQueue};

    #[test]
    fn modifiers_resource_tracks_shift_state() {
        let mut modifiers = ClientInputModifiers::default();
        modifiers.shift = true;
        assert!(modifiers.shift);
    }

    #[test]
    fn intent_order_preserves_shift_before_selection() {
        let mut queue = ClientIntentQueue::default();
        queue.push(ClientIntent::ShiftModifier { pressed: true });
        queue.push(ClientIntent::ToggleUnitSelection {
            unit_id: crate::world::UnitId::new(3),
        });
        let drained = queue.drain();
        assert!(matches!(drained[0], ClientIntent::ShiftModifier { pressed: true }));
        assert!(matches!(
            drained[1],
            ClientIntent::ToggleUnitSelection { .. }
        ));
    }
}
