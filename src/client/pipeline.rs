//! Input → intent collection and client pipeline wiring (ADR-038 U-UI2).

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::{
    BuildModeState, InventoryUiState, PlayerHudHoverState, gameplay_input_blocked_by_hud,
};
use crate::units::UnitRenderEntity;
use crate::units::input::{
    BoxSelectDrag, cursor_screen_position, cursor_world_ray, normalized_screen_rect,
    pick_unit_along_ray, pick_unit_command_target_along_ray, terrain_click_to_world_position,
};
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

/// Flush dispatch trace after intents are applied.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ClientIntentFlushSystems;

/// Registers client intent resources and pipeline systems.
pub struct ClientPipelinePlugin;

impl Plugin for ClientPipelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientIntentQueue>()
            .init_resource::<ClientInputModifiers>()
            .init_resource::<crate::client::inventory_intent::InventoryIntentQueue>()
            .init_resource::<crate::client::commands::ResolvedCommandFeedback>();
    }
}

/// Bundled resources for unit input collection (keeps system param count under Bevy limit).
#[derive(SystemParam)]
pub struct CollectUnitInputParams<'w> {
    pub mouse: Res<'w, ButtonInput<MouseButton>>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub world: Res<'w, WorldData>,
    pub config: Res<'w, WorldConfig>,
    pub unit_catalog: Res<'w, UnitCatalog>,
    pub render_assets: Option<Res<'w, TerrainRenderAssets>>,
    pub queue: ResMut<'w, ClientIntentQueue>,
    pub modifiers: ResMut<'w, ClientInputModifiers>,
    pub box_drag: ResMut<'w, BoxSelectDrag>,
    pub hud_hover: Res<'w, PlayerHudHoverState>,
    pub inventory_ui: Res<'w, InventoryUiState>,
    pub build_mode: Res<'w, BuildModeState>,
}

impl CollectUnitInputParams<'_> {
    fn blocks_world_intents(&self) -> bool {
        gameplay_input_blocked_by_hud(&self.hud_hover)
            || crate::ui::gameplay::inventory_panel_blocks_world_input(&self.inventory_ui)
            || self.build_mode.blocks_gameplay_world_intents()
    }
}

/// Sample modifiers and translate mouse input into intents (no selection/command side effects).
pub fn collect_unit_input_intents(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    units: Query<(&UnitRenderEntity, &GlobalTransform)>,
    mut params: CollectUnitInputParams,
) {
    let shift = params
        .keyboard
        .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if params.modifiers.shift != shift {
        params
            .queue
            .push(ClientIntent::ShiftModifier { pressed: shift });
    }
    params.modifiers.shift = shift;

    if params.blocks_world_intents() {
        return;
    }

    let layout = params.config.chunk_layout();
    let vertical_scale = params
        .render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    if params.mouse.just_pressed(MouseButton::Left) {
        if let Some(screen) = cursor_screen_position(&windows) {
            params.box_drag.begin(screen);
        }
    }

    if params.mouse.pressed(MouseButton::Left) {
        if let Some(screen) = cursor_screen_position(&windows) {
            params.box_drag.update(screen);
        }
    }

    let selection_policy = params.modifiers.selection_policy;

    if params.mouse.just_released(MouseButton::Left) {
        if params.box_drag.active {
            if params.box_drag.is_box_drag() {
                let (rect_min, rect_max) =
                    normalized_screen_rect(params.box_drag.start, params.box_drag.current);
                if shift {
                    params
                        .queue
                        .push(ClientIntent::BoxSelectAdd { rect_min, rect_max });
                } else {
                    params
                        .queue
                        .push(ClientIntent::BoxSelect { rect_min, rect_max });
                }
            } else if let Some(ray) = cursor_world_ray(&windows, &camera) {
                if let Some(unit_id) = pick_unit_along_ray(
                    &ray,
                    &params.world,
                    &params.unit_catalog,
                    &units,
                    selection_policy,
                ) {
                    if shift {
                        params
                            .queue
                            .push(ClientIntent::ToggleUnitSelection { unit_id });
                    } else {
                        params.queue.push(ClientIntent::SelectUnit { unit_id });
                    }
                } else if terrain_click_to_world_position(
                    &ray,
                    &params.world,
                    layout,
                    vertical_scale,
                )
                .is_some()
                    && !shift
                {
                    params.queue.push(ClientIntent::ClearSelection);
                }
            }
        }
        params.box_drag.reset();
    }

    if params.mouse.just_pressed(MouseButton::Right) {
        let Some(ray) = cursor_world_ray(&windows, &camera) else {
            return;
        };

        use crate::client::CommandTarget;

        if let Some(unit_id) =
            pick_unit_command_target_along_ray(&ray, &params.world, &params.unit_catalog, &units)
        {
            params.queue.push(ClientIntent::ContextualCommand {
                target: CommandTarget::Unit { unit_id },
            });
        } else if let Some(click) =
            terrain_click_to_world_position(&ray, &params.world, layout, vertical_scale)
        {
            params.queue.push(ClientIntent::ContextualCommand {
                target: CommandTarget::Terrain {
                    position: click.world_position,
                },
            });
        }
    }
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
        assert!(matches!(
            drained[0],
            ClientIntent::ShiftModifier { pressed: true }
        ));
        assert!(matches!(
            drained[1],
            ClientIntent::ToggleUnitSelection { .. }
        ));
    }
}
