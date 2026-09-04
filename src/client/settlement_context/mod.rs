//! Camera-derived player settlement focus for client UI (Settlement Context 1).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::{CameraControlSystems, RtsCamera};
use crate::terrain::TerrainRenderAssets;
use crate::world::{SettlementId, WorldConfig, WorldData, WorldPosition};

mod focus;
mod resolve;

pub use focus::{
    camera_view_focus_position, derive_camera_focus_position, viewport_center_world_ray,
};
pub use resolve::{
    SettlementFocusConfig, is_player_manageable_settlement, resolve_focused_player_settlement,
};

/// Client-local answer to "which player settlement is the camera looking at?"
#[derive(Resource, Debug, Clone, PartialEq, Default, Reflect)]
#[reflect(Resource)]
pub struct CameraSettlementContext {
    pub focused_settlement_id: Option<SettlementId>,
    pub focus_world_position: Option<WorldPosition>,
}

/// Systems that maintain [`CameraSettlementContext`].
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct SettlementContextSystems;

pub struct SettlementContextPlugin;

impl Plugin for SettlementContextPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CameraSettlementContext>()
            .init_resource::<CameraSettlementContext>()
            .init_resource::<SettlementFocusConfig>()
            .configure_sets(Update, SettlementContextSystems)
            .add_systems(
                Update,
                update_camera_settlement_context
                    .after(CameraControlSystems)
                    .in_set(SettlementContextSystems),
            );
    }
}

pub fn update_camera_settlement_context(
    world: Res<WorldData>,
    config: Res<SettlementFocusConfig>,
    config_layout: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    mut context: ResMut<CameraSettlementContext>,
) {
    let layout = config_layout.chunk_layout();
    let previous = context.focused_settlement_id;
    let focus =
        derive_camera_focus_position(&windows, &camera, &world, layout, render_assets.as_deref());
    let focused = focus.and_then(|position| {
        resolve_focused_player_settlement(&world, position, previous, config.as_ref())
    });
    if context.focused_settlement_id == focused && context.focus_world_position == focus {
        return;
    }
    context.focused_settlement_id = focused;
    context.focus_world_position = focus;
}
