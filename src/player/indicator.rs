use bevy::prelude::*;

use crate::units::{UnitRenderIndex, UnitSelectionIndicator};
use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, UnitId, WorldData};

/// Tracks one selection ring entity per selected unit.
#[derive(Resource, Default, Debug)]
pub struct UnitSelectionIndicatorState {
    indicators: std::collections::HashMap<UnitId, Entity>,
}

/// Show a green ring at the feet of every locally selected unit (SC2-style, U9).
pub fn sync_unit_selection_indicators(
    mut commands: Commands,
    time: Res<Time>,
    selection: Res<SelectedUnits>,
    index: Res<UnitRenderIndex>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    mut state: ResMut<UnitSelectionIndicatorState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fade_query: Query<(
        &mut SelectionRingFade,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let selected = &selection.0;

    state.indicators.retain(|unit_id, entity| {
        let alive = fade_query.get(*entity).is_ok();
        if !selected.contains(unit_id) {
            if alive {
                commands.entity(*entity).despawn();
            }
            return false;
        }
        // Parent render entity may have been respawned, despawning the child ring.
        alive
    });

    for &unit_id in selected {
        if state.indicators.contains_key(&unit_id) {
            continue;
        }
        let Some(&render_entity) = index.0.get(&unit_id) else {
            continue;
        };
        if world.get_unit(unit_id).is_none() {
            continue;
        }
        let indicator = spawn_indicator(
            &mut commands,
            render_entity,
            unit_id,
            &world,
            &catalog,
            &mut meshes,
            &mut materials,
        );
        state.indicators.insert(unit_id, indicator);
    }

    for (&unit_id, &entity) in &state.indicators {
        if !selected.contains(&unit_id) {
            continue;
        }
        if let Ok((mut fade, material)) = fade_query.get_mut(entity) {
            fade.elapsed_secs += time.delta_secs();
            let fade_in = (fade.elapsed_secs / SELECTION_FADE_IN_SECS).clamp(0.0, 1.0);
            if let Some(material) = materials.get_mut(&material.0) {
                material.base_color.set_alpha(0.15 + 0.7 * fade_in);
            }
        }
    }
}

#[derive(Component, Debug)]
pub(crate) struct SelectionRingFade {
    elapsed_secs: f32,
}

const SELECTION_FADE_IN_SECS: f32 = 0.12;

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
        base_color: Color::srgba(0.15, 0.95, 0.25, 0.15),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands
        .spawn((
            UnitSelectionIndicator,
            SelectionRingFade {
                elapsed_secs: 0.0,
            },
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
