use bevy::prelude::*;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::world::{ChunkLayout, WorldPosition};

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct PendingNewGameCameraFocus {
    pub world_position: WorldPosition,
}

const NEW_GAME_CAMERA_DISTANCE: f32 = 90.0;

pub fn focus_camera_on_new_game_spawn(
    mut commands: Commands,
    pending: Option<Res<PendingNewGameCameraFocus>>,
    world: Res<crate::world::WorldData>,
    mut camera: Query<&mut RtsCameraState, With<RtsCamera>>,
) {
    let Some(pending) = pending else {
        return;
    };
    let global = pending.world_position.to_global(world.layout());
    let focus = Vec3::new(global.x, 0.0, global.z);
    for mut state in &mut camera {
        state.target_focus = focus;
        state.target_distance = NEW_GAME_CAMERA_DISTANCE;
    }
    commands.remove_resource::<PendingNewGameCameraFocus>();
}

pub fn pending_camera_focus_for_anchor(
    anchor: &crate::world::OriginSpawnAnchor,
    layout: ChunkLayout,
) -> PendingNewGameCameraFocus {
    let global = Vec3::new(anchor.start_x, 0.0, anchor.start_z);
    PendingNewGameCameraFocus {
        world_position: WorldPosition::from_global(global, layout),
    }
}
