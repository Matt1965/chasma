use bevy::prelude::*;

use crate::terrain::TerrainRenderAssets;
use crate::units::input::SelectedUnits;
use crate::units::{UnitRenderIndex, UnitSelectionIndicator};
use crate::world::{UnitCatalog, UnitId, WorldConfig, WorldData};

use super::selection_ring_mesh::{build_terrain_selection_ring_mesh, selection_ring_radius};

/// Tracks one selection ring entity per selected unit.
#[derive(Resource, Default, Debug)]
pub struct UnitSelectionIndicatorState {
    indicators: std::collections::HashMap<UnitId, Entity>,
}

#[derive(Component, Debug)]
pub(crate) struct TerrainSelectionRing {
    unit_id: UnitId,
}

/// Show a green ring at the feet of every locally selected unit (SC2-style, U9).
pub fn sync_unit_selection_indicators(
    mut commands: Commands,
    time: Res<Time>,
    selection: Res<SelectedUnits>,
    index: Res<UnitRenderIndex>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    catalog: Res<UnitCatalog>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut state: ResMut<UnitSelectionIndicatorState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fade_query: Query<(&mut SelectionRingFade, &MeshMaterial3d<StandardMaterial>)>,
    mut rings: Query<(&TerrainSelectionRing, &Mesh3d)>,
) {
    let selected = &selection.0;
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    state.indicators.retain(|unit_id, entity| {
        let alive = fade_query.get(*entity).is_ok();
        if !selected.contains(unit_id) {
            if alive {
                commands.entity(*entity).despawn();
            }
            return false;
        }
        alive
    });

    for &unit_id in selected {
        if state.indicators.contains_key(&unit_id) {
            continue;
        }
        let Some(&render_entity) = index.0.get(&unit_id) else {
            continue;
        };
        let Some(record) = world.get_unit(unit_id) else {
            continue;
        };
        let parent_global = record.placement.position.to_global(layout);
        let indicator = spawn_indicator(
            &mut commands,
            render_entity,
            unit_id,
            parent_global,
            &world,
            &catalog,
            layout,
            vertical_scale,
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

        let Some(record) = world.get_unit(unit_id) else {
            continue;
        };
        let parent_global = record.placement.position.to_global(layout);
        let radius = selection_ring_radius(&world, &catalog, unit_id);
        if let Ok((ring, mesh3d)) = rings.get(entity) {
            if ring.unit_id != unit_id {
                continue;
            }
            if let Some(mesh) = meshes.get_mut(&mesh3d.0) {
                *mesh = build_terrain_selection_ring_mesh(
                    parent_global,
                    radius * 0.82,
                    radius,
                    &world,
                    layout,
                    vertical_scale,
                );
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
    parent_global: Vec3,
    world: &WorldData,
    catalog: &UnitCatalog,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let radius = selection_ring_radius(world, catalog, unit_id);
    let mesh = meshes.add(build_terrain_selection_ring_mesh(
        parent_global,
        radius * 0.82,
        radius,
        world,
        layout,
        vertical_scale,
    ));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.15, 0.95, 0.25, 0.15),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });

    commands
        .spawn((
            UnitSelectionIndicator,
            TerrainSelectionRing { unit_id },
            SelectionRingFade { elapsed_secs: 0.0 },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Visibility::default(),
            ChildOf(parent),
        ))
        .id()
}
