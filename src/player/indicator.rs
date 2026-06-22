use bevy::prelude::*;

use crate::units::{UnitRenderIndex, UnitSelectionIndicator};
use crate::world::{UnitCatalog, UnitId, WorldData};

use super::selection::PlayerUnitSelection;

/// Tracks the spawned selection ring entity for the current selection.
#[derive(Resource, Default, Debug)]
pub struct UnitSelectionIndicatorState {
    pub tracked_unit: Option<UnitId>,
    pub indicator: Option<Entity>,
}

/// Show a green ring at the feet of the locally selected unit (SC2-style, U8).
pub fn sync_unit_selection_indicator(
    mut commands: Commands,
    selection: Res<PlayerUnitSelection>,
    index: Res<UnitRenderIndex>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    mut state: ResMut<UnitSelectionIndicatorState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    indicators: Query<Entity, With<UnitSelectionIndicator>>,
) {
    if selection.selected != state.tracked_unit {
        despawn_indicator(&mut commands, &mut state, &indicators);
        state.tracked_unit = selection.selected;

        if let Some(unit_id) = selection.selected {
            if let Some(&render_entity) = index.0.get(&unit_id) {
                state.indicator = Some(spawn_indicator(
                    &mut commands,
                    render_entity,
                    unit_id,
                    &world,
                    &catalog,
                    &mut meshes,
                    &mut materials,
                ));
            }
        }
        return;
    }

    if selection.selected.is_some() && state.indicator.is_none() {
        let Some(unit_id) = selection.selected else {
            return;
        };
        if let Some(&render_entity) = index.0.get(&unit_id) {
            state.indicator = Some(spawn_indicator(
                &mut commands,
                render_entity,
                unit_id,
                &world,
                &catalog,
                &mut meshes,
                &mut materials,
            ));
        }
    }
}

fn despawn_indicator(
    commands: &mut Commands,
    state: &mut UnitSelectionIndicatorState,
    indicators: &Query<Entity, With<UnitSelectionIndicator>>,
) {
    if let Some(entity) = state.indicator.take() {
        commands.entity(entity).despawn();
    }
    for entity in indicators {
        commands.entity(entity).despawn();
    }
}

fn spawn_indicator(
    commands: &mut Commands,
    parent: Entity,
    unit_id: UnitId,
    world: &WorldData,
    catalog: &UnitCatalog,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let radius = selection_ring_radius(world, catalog, unit_id);
    let mesh = meshes.add(Annulus::new(radius * 0.82, radius));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.15, 0.95, 0.25, 0.85),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands
        .spawn((
            UnitSelectionIndicator,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_translation(Vec3::new(0.0, 0.08, 0.0)),
            Visibility::default(),
            ChildOf(parent),
        ))
        .id()
}

fn selection_ring_radius(world: &WorldData, catalog: &UnitCatalog, unit_id: UnitId) -> f32 {
    let Some(record) = world.get_unit(unit_id) else {
        return 1.0;
    };
    let Some(definition) = catalog.get(&record.definition_id) else {
        return 1.0;
    };
    (definition.collision_radius_meters * 2.0).max(0.9)
}
