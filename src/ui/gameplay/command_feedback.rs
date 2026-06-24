//! Move command ground marker and command ping (ADR-040 U-UI4).

use bevy::prelude::*;

use crate::world::{ChunkLayout, WorldPosition};

/// Ground marker at the last move-click destination (gameplay presentation).
#[derive(Resource, Default, Debug)]
pub struct MoveCommandFeedback {
    marker: Option<MoveMarkerState>,
    ping: Option<CommandPingState>,
}

#[derive(Debug, Clone)]
struct MoveMarkerState {
    render_position: Vec3,
    remaining_secs: f32,
}

#[derive(Debug, Clone)]
struct CommandPingState {
    render_position: Vec3,
    elapsed_secs: f32,
}

impl MoveCommandFeedback {
    pub fn set_target(&mut self, target: WorldPosition, layout: ChunkLayout, vertical_scale: f32) {
        let global = target.to_global(layout);
        let render_position = Vec3::new(global.x, global.y * vertical_scale, global.z);
        self.marker = Some(MoveMarkerState {
            render_position,
            remaining_secs: MOVE_MARKER_LIFETIME_SECS,
        });
        self.ping = Some(CommandPingState {
            render_position,
            elapsed_secs: 0.0,
        });
    }

    pub fn has_active_marker(&self) -> bool {
        self.marker.is_some()
    }
}

const MOVE_MARKER_LIFETIME_SECS: f32 = 2.0;
const MOVE_MARKER_FADE_IN_SECS: f32 = 0.12;
const COMMAND_PING_DURATION_SECS: f32 = 0.45;
const COMMAND_PING_MAX_SCALE: f32 = 1.8;

/// Marker mesh spawned at the move destination.
#[derive(Component, Debug)]
pub struct MoveCommandIndicator;

#[derive(Component, Debug)]
pub(crate) struct MoveCommandIndicatorFade {
    elapsed_secs: f32,
}

/// Expanding pulse ring at command issue location.
#[derive(Component, Debug)]
pub struct CommandPingIndicator;

#[derive(Component, Debug)]
pub(crate) struct CommandPingPulse {
    elapsed_secs: f32,
}

/// Spawn or refresh the destination marker when feedback state is set.
pub fn sync_move_command_indicator(
    mut commands: Commands,
    feedback: Res<MoveCommandFeedback>,
    mut marker_entity: Local<Option<Entity>>,
    mut ping_entity: Local<Option<Entity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(marker) = feedback.marker.as_ref() else {
        despawn_if_present(&mut commands, &mut marker_entity);
        despawn_if_present(&mut commands, &mut ping_entity);
        return;
    };

    if marker_entity.is_none() {
        let mesh = meshes.add(Circle::new(0.65));
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.85, 1.0, 0.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let entity = commands
            .spawn((
                MoveCommandIndicator,
                MoveCommandIndicatorFade {
                    elapsed_secs: 0.0,
                },
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                    .with_translation(marker.render_position + Vec3::new(0.0, 0.12, 0.0)),
                Visibility::default(),
            ))
            .id();
        *marker_entity = Some(entity);
    } else if let Some(entity) = *marker_entity {
        commands.entity(entity).insert(
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_translation(marker.render_position + Vec3::new(0.0, 0.12, 0.0)),
        );
    }

    if feedback.ping.is_some() && ping_entity.is_none() {
        let mesh = meshes.add(Annulus::new(0.35, 0.55));
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.95, 1.0, 0.65),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let entity = commands
            .spawn((
                CommandPingIndicator,
                CommandPingPulse {
                    elapsed_secs: 0.0,
                },
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                    .with_translation(marker.render_position + Vec3::new(0.0, 0.14, 0.0)),
                Visibility::default(),
            ))
            .id();
        *ping_entity = Some(entity);
    }
}

fn despawn_if_present(commands: &mut Commands, entity: &mut Local<Option<Entity>>) {
    if let Some(id) = entity.take() {
        commands.entity(id).despawn();
    }
}

/// Fade marker and pulse; expire after lifetime.
pub fn tick_move_command_indicator(
    time: Res<Time>,
    mut feedback: ResMut<MoveCommandFeedback>,
    mut markers: Query<(
        Entity,
        &mut MoveCommandIndicatorFade,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut pings: Query<(
        Entity,
        &mut CommandPingPulse,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if let Some(marker) = feedback.marker.as_mut() {
        marker.remaining_secs -= time.delta_secs();
        if marker.remaining_secs <= 0.0 {
            feedback.marker = None;
        }
    }

    if let Some(ping) = feedback.ping.as_mut() {
        ping.elapsed_secs += time.delta_secs();
        if ping.elapsed_secs >= COMMAND_PING_DURATION_SECS {
            feedback.ping = None;
        }
    }

    for (_entity, mut fade, material) in &mut markers {
        fade.elapsed_secs += time.delta_secs();
        let Some(material) = materials.get_mut(&material.0) else {
            continue;
        };
        let lifetime_alpha = feedback
            .marker
            .as_ref()
            .map(|marker| (marker.remaining_secs / MOVE_MARKER_LIFETIME_SECS).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let fade_in = (fade.elapsed_secs / MOVE_MARKER_FADE_IN_SECS).clamp(0.0, 1.0);
        material.base_color.set_alpha(0.55 * fade_in * lifetime_alpha);
    }

    for (_entity, mut pulse, mut transform, material) in &mut pings {
        pulse.elapsed_secs += time.delta_secs();
        let t = (pulse.elapsed_secs / COMMAND_PING_DURATION_SECS).clamp(0.0, 1.0);
        let scale = 1.0 + t * (COMMAND_PING_MAX_SCALE - 1.0);
        transform.scale = Vec3::splat(scale);
        if let Some(material) = materials.get_mut(&material.0) {
            material.base_color.set_alpha(0.65 * (1.0 - t));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_feedback_records_marker_on_set_target() {
        use crate::world::{ChunkCoord, ChunkLayout, LocalPosition};

        let mut feedback = MoveCommandFeedback::default();
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let target = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(12.0, 0.0, 8.0)),
        );
        feedback.set_target(target, layout, 1.0);
        assert!(feedback.has_active_marker());
    }
}
