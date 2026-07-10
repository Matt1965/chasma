//! Overhead unit health bar presentation (ADR-062 C9).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::debug::{
    CommandTraceBuffer, CommandTraceIntentKind, CommandTraceOutcome, DebugOverlaySettings,
};
use crate::ui::gameplay::GameplayHoveredUnit;
use crate::units::input::SelectedUnits;
use crate::units::spawn::UnitRenderIndex;
use crate::world::{is_unit_alive, UnitCatalog, UnitId, WorldData};

use super::visibility::{health_percent, should_show_health_bar};

/// Marker for a unit overhead health bar root.
#[derive(Component, Debug)]
pub struct UnitHealthBar {
    pub unit_id: UnitId,
}

#[derive(Component, Debug)]
pub(crate) struct HealthBarBackground;

#[derive(Component, Debug)]
pub(crate) struct HealthBarFill;

#[derive(Debug, Clone, Copy)]
struct HealthBarEntities {
    root: Entity,
    fill: Entity,
}

#[derive(Resource, Default, Debug)]
pub struct UnitHealthBarState {
    bars: std::collections::HashMap<UnitId, HealthBarEntities>,
}

const BAR_MESH_WIDTH: f32 = 1.0;
const BAR_MESH_HEIGHT: f32 = 0.12;
const BAR_MESH_DEPTH: f32 = 0.05;

/// Bundled inputs to stay within Bevy system parameter limits.
#[derive(SystemParam)]
pub struct HealthBarSyncParams<'w> {
    pub world: Res<'w, WorldData>,
    pub catalog: Res<'w, UnitCatalog>,
    pub selection: Res<'w, SelectedUnits>,
    pub hover: Res<'w, GameplayHoveredUnit>,
    pub debug: Res<'w, DebugOverlaySettings>,
    pub index: Res<'w, UnitRenderIndex>,
    pub state: ResMut<'w, UnitHealthBarState>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
    pub trace: ResMut<'w, CommandTraceBuffer>,
}

pub fn health_bar_color(percent: f32) -> Color {
    if percent > 0.6 {
        Color::srgba(0.2, 0.95, 0.3, 1.0)
    } else if percent > 0.3 {
        Color::srgba(1.0, 0.88, 0.15, 1.0)
    } else {
        Color::srgba(1.0, 0.2, 0.15, 1.0)
    }
}

fn health_bar_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive: (color.to_linear() * 2.5).into(),
        unlit: true,
        alpha_mode: AlphaMode::Opaque,
        ..default()
    }
}

/// Sync overhead health bars from authoritative [`WorldData`] vitals.
pub fn sync_unit_health_bars(
    mut commands: Commands,
    mut params: HealthBarSyncParams,
    bar_alive: Query<Entity, With<UnitHealthBar>>,
    mut fills: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<HealthBarFill>>,
) {
    let dev_show_all = params.debug.health;

    let mut desired = std::collections::HashSet::new();
    for &unit_id in params.index.0.keys() {
        if should_show_health_bar(
            unit_id,
            &params.world,
            &params.selection,
            params.hover.unit_id,
            dev_show_all,
        ) {
            desired.insert(unit_id);
        }
    }

    state_bars_retain(&mut commands, &mut params, &desired, &bar_alive);

    for unit_id in desired {
        let Some(record) = params.world.get_unit(unit_id) else {
            continue;
        };
        if !is_unit_alive(record) {
            continue;
        }
        let percent = health_percent(record.vitals.current_hp, record.vitals.max_hp);
        let definition_id = record.definition_id.clone();

        if let Some(entities) = params.state.bars.get(&unit_id) {
            if let Ok((mut transform, material)) = fills.get_mut(entities.fill) {
                transform.scale.x = percent.max(0.04);
                transform.translation.x = -BAR_MESH_WIDTH * 0.5 + (BAR_MESH_WIDTH * percent) * 0.5;
                if let Some(mat) = params.materials.get_mut(&material.0) {
                    *mat = health_bar_material(health_bar_color(percent));
                }
            }
            continue;
        }

        let Some(&render_entity) = params.index.0.get(&unit_id) else {
            continue;
        };

        spawn_health_bar(
            &mut commands,
            &mut params,
            unit_id,
            render_entity,
            &definition_id,
            percent,
        );
    }
}

fn state_bars_retain(
    commands: &mut Commands,
    params: &mut HealthBarSyncParams,
    desired: &std::collections::HashSet<UnitId>,
    bar_alive: &Query<Entity, With<UnitHealthBar>>,
) {
    let trace_health = params.debug.health;
    params.state.bars.retain(|unit_id, entities| {
        let keep = desired.contains(unit_id)
            && params.index.0.contains_key(unit_id)
            && bar_alive.get(entities.root).is_ok();
        if keep {
            return true;
        }
        if bar_alive.get(entities.root).is_ok() {
            commands.entity(entities.root).despawn();
        }
        if trace_health {
            push_health_bar_trace(
                &mut params.trace,
                *unit_id,
                CommandTraceOutcome::HealthBarHidden,
            );
        }
        false
    });
}

fn spawn_health_bar(
    commands: &mut Commands,
    params: &mut HealthBarSyncParams,
    unit_id: UnitId,
    render_entity: Entity,
    definition_id: &crate::world::UnitDefinitionId,
    percent: f32,
) {
    let (y_offset, width_scale) = bar_layout(&params.catalog, definition_id);

    let bg_mesh = params
        .meshes
        .add(Cuboid::new(BAR_MESH_WIDTH, BAR_MESH_HEIGHT, BAR_MESH_DEPTH));
    let fill_mesh = params.meshes.add(Cuboid::new(
        BAR_MESH_WIDTH,
        BAR_MESH_HEIGHT * 0.85,
        BAR_MESH_DEPTH * 1.1,
    ));
    let bg_material = params.materials.add(StandardMaterial {
        base_color: Color::srgba(0.02, 0.02, 0.02, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });
    let fill_material = params
        .materials
        .add(health_bar_material(health_bar_color(percent)));

    let mut fill_entity = None;
    let root = commands
        .spawn((
            UnitHealthBar { unit_id },
            Transform::from_translation(Vec3::new(0.0, y_offset, 0.0))
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::new(width_scale, 1.0, 1.0)),
            Visibility::Visible,
            ChildOf(render_entity),
        ))
        .with_children(|parent| {
            parent.spawn((
                HealthBarBackground,
                Mesh3d(bg_mesh),
                MeshMaterial3d(bg_material),
                Transform::default(),
                Visibility::Visible,
            ));
            fill_entity = Some(
                parent
                    .spawn((
                        HealthBarFill,
                        Mesh3d(fill_mesh),
                        MeshMaterial3d(fill_material),
                        Transform::from_translation(Vec3::new(
                            -BAR_MESH_WIDTH * 0.5 + (BAR_MESH_WIDTH * percent) * 0.5,
                            0.0,
                            0.02,
                        ))
                        .with_scale(Vec3::new(percent.max(0.04), 1.0, 1.0)),
                        Visibility::Visible,
                    ))
                    .id(),
            );
        })
        .id();

    params.state.bars.insert(
        unit_id,
        HealthBarEntities {
            root,
            fill: fill_entity.expect("health bar fill spawned"),
        },
    );
    if params.debug.health {
        push_health_bar_trace(&mut params.trace, unit_id, CommandTraceOutcome::HealthBarShown);
    }
}

fn bar_layout(
    catalog: &UnitCatalog,
    definition_id: &crate::world::UnitDefinitionId,
) -> (f32, f32) {
    let radius = catalog
        .get(definition_id)
        .map(|def| def.collision_radius_meters)
        .unwrap_or(0.5);
    let y_offset = (radius * 2.8).max(1.2);
    let width_scale = (radius * 3.5).max(1.4);
    (y_offset, width_scale)
}

fn push_health_bar_trace(
    trace: &mut CommandTraceBuffer,
    unit_id: UnitId,
    outcome: CommandTraceOutcome,
) {
    trace.push_presentation_entry(0, CommandTraceIntentKind::HealthBar, vec![unit_id], outcome);
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Color;

    use super::super::visibility::health_percent;
    use super::health_bar_color;

    #[test]
    fn health_bar_color_tracks_percent() {
        assert_eq!(health_bar_color(0.8), Color::srgba(0.2, 0.95, 0.3, 1.0));
        assert_eq!(health_bar_color(0.1), Color::srgba(1.0, 0.2, 0.15, 1.0));
    }

    #[test]
    fn health_percent_matches_visibility_helper() {
        assert!((health_percent(1, 4) - 0.25).abs() < f32::EPSILON);
    }
}
