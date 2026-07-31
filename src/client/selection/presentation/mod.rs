//! Shared world-selection presentation (Dev UI Revamp Slice 2).
//!
//! Observes [`WorldSelectionState`] and [`SelectedUnits`]; spawns client-local outline
//! entities only. Does not mutate world data or selection authority.

mod geometry;
mod materials;
mod resolve;

pub use geometry::{
    OUTLINE_LIFT_METERS, SELECTION_GREEN, SELECTION_GREEN_PRIMARY, build_flat_annulus_mesh,
    build_rect_frame_mesh, build_selection_outline_mesh,
};
pub use materials::SelectionPresentationMaterials;
pub use resolve::{
    ITEM_PILE_SELECTION_MIN_RADIUS_METERS, ResolvedSelectionFootprint,
    WorldObjectPresentationTarget, occupied_cells_for_resolved,
    resolve_building_selection_footprint, resolve_doodad_selection_footprint_with_collision,
    resolve_item_pile_selection_footprint, resolve_world_object_footprint,
};

use bevy::prelude::*;

use crate::item_piles::ItemPilePresentationSettings;
use crate::terrain::TerrainRenderAssets;
use crate::world::{
    BuildingCatalog, DoodadCatalog, FootprintCatalog, ItemPileSettings, WorldConfig, WorldData,
};

use super::WorldSelectionState;

/// Marker on non-unit selection outline entities.
#[derive(Component, Debug, Clone, Copy)]
pub struct WorldSelectionOutline {
    pub target: WorldObjectPresentationTarget,
}

/// Tracks the single active world-object selection outline (building, doodad, or pile).
#[derive(Resource, Default, Debug)]
pub struct WorldObjectSelectionPresentationState {
    active_target: Option<WorldObjectPresentationTarget>,
    entity: Option<Entity>,
}

impl WorldObjectSelectionPresentationState {
    pub fn active_target(&self) -> Option<WorldObjectPresentationTarget> {
        self.active_target
    }

    pub fn clear(&mut self, commands: &mut Commands) {
        if let Some(entity) = self.entity.take() {
            commands.entity(entity).despawn();
        }
        self.active_target = None;
    }
}

/// Sync building / doodad / item-pile selection outlines from shared selection authority.
pub fn sync_world_object_selection_presentation(
    mut commands: Commands,
    world_selection: Res<WorldSelectionState>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    building_catalog: Res<BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    pile_settings: Res<ItemPileSettings>,
    pile_presentation: Res<ItemPilePresentationSettings>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut state: ResMut<WorldObjectSelectionPresentationState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material_cache: ResMut<SelectionPresentationMaterials>,
    mut outlines: Query<(&WorldSelectionOutline, &mut Transform, &Mesh3d)>,
) {
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    let desired = WorldObjectPresentationTarget::from_selection(
        world_selection.category,
        world_selection.building_id,
        world_selection.doodad_id,
        world_selection.pile_id,
    );

    if desired != state.active_target {
        state.clear(&mut commands);
        state.active_target = desired;

        if let Some(target) = desired {
            let Some(footprint) = resolve_world_object_footprint(
                target,
                &world,
                &building_catalog,
                &footprint_catalog,
                &doodad_catalog,
                &pile_settings,
                &pile_presentation,
                layout,
                vertical_scale,
            ) else {
                state.active_target = None;
                return;
            };

            let mesh = meshes.add(build_selection_outline_mesh(
                &footprint,
                &world,
                layout,
                vertical_scale,
            ));
            let material = material_cache.outline(&mut materials);
            let transform = outline_transform(&footprint);
            let entity = commands
                .spawn((
                    WorldSelectionOutline { target },
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    transform,
                    Visibility::default(),
                ))
                .id();
            state.entity = Some(entity);
        }
        return;
    }

    let Some(target) = desired else {
        return;
    };
    let Some(entity) = state.entity else {
        return;
    };
    let Some(footprint) = resolve_world_object_footprint(
        target,
        &world,
        &building_catalog,
        &footprint_catalog,
        &doodad_catalog,
        &pile_settings,
        &pile_presentation,
        layout,
        vertical_scale,
    ) else {
        state.clear(&mut commands);
        return;
    };

    if let Ok((marker, mut transform, mesh3d)) = outlines.get_mut(entity) {
        if marker.target != target {
            return;
        }
        *transform = outline_transform(&footprint);
        if let Some(mesh) = meshes.get_mut(&mesh3d.0) {
            *mesh = build_selection_outline_mesh(&footprint, &world, layout, vertical_scale);
        }
    }
}

fn outline_transform(footprint: &ResolvedSelectionFootprint) -> Transform {
    if footprint.terrain_conforming {
        Transform::from_translation(footprint.anchor_render)
    } else {
        Transform {
            translation: footprint.anchor_render,
            rotation: Quat::from_rotation_y(footprint.yaw_radians),
            scale: Vec3::ONE,
        }
    }
}

mod tests;
