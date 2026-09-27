//! Client-local model ghosts for dev catalog placement (presentation only).

use bevy::asset::LoadState;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::buildings::BuildingSceneAssets;
use crate::buildings::components::OriginalBuildingMaterial;
use crate::dev::dev_mode::{DefinitionId, DevModeState};
use crate::doodads::DoodadSceneAssets;
use crate::item_piles::{ItemPileFallbackAssets, ItemPilePresentationSettings, item_pile_visual_rotation};
use crate::item_piles::ItemSceneAssets;
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::units::UnitSceneAssets;
use crate::world::{
    BuildingCatalog, BuildingPlacement, DoodadCatalog, DoodadRenderKey, FixedScale, ItemCatalog,
    UnitCatalog, WorldConfig, WorldData, building_anchor_render_transform,
    building_model_child_local_transform, building_model_render_transform, doodad_final_render_scale,
    ground_and_quantize_building_anchor, unit_definition_visual_scale, unit_visual_rotation,
};

use super::preview::{DevPlacementPreview, PreviewPoint};
use super::preview_visual::{
    DevPlacementModelVisual, dev_placement_model_preview_active, dev_placement_model_visual_key,
    item_catalog_preview_armed, resolve_dev_placement_model_visual,
};

/// Root entity for one dev placement model ghost instance.
#[derive(Component, Debug)]
pub struct DevPlacementModelGhost {
    pub slot: usize,
}

#[derive(Component, Debug)]
pub(crate) struct DevPlacementModelGhostRoot;

#[derive(Component, Debug)]
pub(crate) struct DevPlacementModelTintPending;

#[derive(Resource, Debug, Default)]
pub(crate) struct DevPlacementModelPreviewState {
    visual_key: Option<String>,
}

pub(crate) fn init_dev_placement_model_preview(app: &mut App) {
    app.init_resource::<DevPlacementModelPreviewState>();
}

#[derive(SystemParam)]
pub(crate) struct DevPlacementModelPreviewCatalogs<'w> {
    unit_catalog: Res<'w, UnitCatalog>,
    doodad_catalog: Res<'w, DoodadCatalog>,
    building_catalog: Res<'w, BuildingCatalog>,
    item_catalog: Res<'w, ItemCatalog>,
    pile_settings: Res<'w, ItemPilePresentationSettings>,
}

#[derive(SystemParam)]
pub(crate) struct DevPlacementModelPreviewAssets<'w> {
    unit_scenes: ResMut<'w, UnitSceneAssets>,
    doodad_scenes: ResMut<'w, DoodadSceneAssets>,
    building_scenes: ResMut<'w, BuildingSceneAssets>,
    item_scenes: ResMut<'w, ItemSceneAssets>,
    pile_fallback: ResMut<'w, ItemPileFallbackAssets>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    state: ResMut<'w, DevPlacementModelPreviewState>,
}

#[derive(SystemParam)]
pub(crate) struct DevPlacementModelPreviewQueries<'w, 's> {
    ghosts: Query<'w, 's, (Entity, &'static DevPlacementModelGhost)>,
    pending: Query<'w, 's, Entity, With<DevPlacementModelTintPending>>,
    children: Query<'w, 's, &'static Children>,
    mesh_materials: Query<'w, 's, &'static MeshMaterial3d<StandardMaterial>>,
    originals: Query<'w, 's, &'static OriginalBuildingMaterial>,
}

/// Sync translucent catalog model ghosts to [`DevPlacementPreview`] points.
pub(crate) fn sync_dev_placement_model_previews(
    mut commands: Commands,
    dev_state: Res<DevModeState>,
    preview: Res<DevPlacementPreview>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    asset_server: Res<AssetServer>,
    catalogs: DevPlacementModelPreviewCatalogs,
    mut assets: DevPlacementModelPreviewAssets,
    render_assets: Option<Res<TerrainRenderAssets>>,
    queries: DevPlacementModelPreviewQueries,
) {
    let DevPlacementModelPreviewCatalogs {
        unit_catalog,
        doodad_catalog,
        building_catalog,
        item_catalog,
        pile_settings,
    } = catalogs;
    let DevPlacementModelPreviewQueries {
        ghosts,
        pending,
        children,
        mesh_materials,
        originals,
    } = queries;
    let mut clear_all = || {
        for (entity, _) in &ghosts {
            commands.entity(entity).despawn();
        }
        assets.state.visual_key = None;
    };

    if !dev_placement_model_preview_active(&dev_state, &preview) {
        clear_all();
        return;
    }
    let definition = dev_state.selected_definition.as_ref().unwrap();
    if matches!(definition, DefinitionId::Item(_)) && !item_catalog_preview_armed(&dev_state) {
        clear_all();
        return;
    }
    let Some(visual) = resolve_dev_placement_model_visual(
        definition,
        &unit_catalog,
        &doodad_catalog,
        &building_catalog,
        &item_catalog,
    ) else {
        clear_all();
        return;
    };
    if preview.points.is_empty() {
        clear_all();
        return;
    }

    let visual_key = dev_placement_model_visual_key(&dev_state, definition);
    let ghost_count = ghosts.iter().len();
    let slots_match = ghost_count == preview.points.len()
        && ghosts
            .iter()
            .all(|(_, marker)| marker.slot < preview.points.len());
    let needs_rebuild = assets.state.visual_key.as_deref() != Some(visual_key.as_str())
        || !slots_match;

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let yaw = dev_state.placement_yaw_deg;
    let uniform_scale = dev_state.placement_uniform_scale.max(0.01);

    if needs_rebuild {
        for (entity, _) in &ghosts {
            commands.entity(entity).despawn();
        }
        assets.state.visual_key = Some(visual_key);
        for slot in 0..preview.points.len() {
            let point = preview.points[slot];
            let transform = placement_transform(
                definition,
                &visual,
                point,
                &world,
                &unit_catalog,
                &doodad_catalog,
                &building_catalog,
                &item_catalog,
                &pile_settings,
                layout,
                vertical_scale,
                yaw,
                uniform_scale,
            );
            if let Some(entity) = spawn_ghost_root(
                &mut commands,
                &visual,
                slot,
                transform,
                &asset_server,
                &mut assets.unit_scenes,
                &mut assets.doodad_scenes,
                &mut assets.building_scenes,
                &mut assets.item_scenes,
                &building_catalog,
                &item_catalog,
                &pile_settings,
                &mut assets.pile_fallback,
                &mut assets.meshes,
                &mut assets.materials,
                definition,
                uniform_scale,
            ) {
                commands.entity(entity).insert(DevPlacementModelTintPending);
            }
        }
        tint_pending_previews(
            &mut commands,
            &pending,
            &children,
            &mesh_materials,
            &originals,
            &mut assets.materials,
        );
        return;
    }

    for (entity, marker) in &ghosts {
        let point = preview.points[marker.slot];
        let transform = placement_transform(
            definition,
            &visual,
            point,
            &world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &item_catalog,
            &pile_settings,
            layout,
            vertical_scale,
            yaw,
            uniform_scale,
        );
        commands.entity(entity).insert(transform);
    }

    tint_pending_previews(
        &mut commands,
        &pending,
        &children,
        &mesh_materials,
        &originals,
        &mut assets.materials,
    );
}

fn placement_transform(
    definition: &DefinitionId,
    visual: &DevPlacementModelVisual,
    point: PreviewPoint,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    item_catalog: &ItemCatalog,
    pile_settings: &ItemPilePresentationSettings,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
    yaw_deg: f32,
    uniform_scale: f32,
) -> Transform {
    let position = point.position;
    match definition {
        DefinitionId::Unit(id) => {
            let definition = unit_catalog.get(id);
            let translation = world_position_to_render_global(position, layout, vertical_scale);
            let scale = definition
                .map(unit_definition_visual_scale)
                .unwrap_or(Vec3::ONE);
            Transform {
                translation,
                rotation: definition
                    .map(|def| unit_visual_rotation(def, Quat::IDENTITY))
                    .unwrap_or(Quat::IDENTITY),
                scale,
            }
        }
        DefinitionId::Doodad(id) => {
            let rotation = Quat::from_rotation_y(yaw_deg.to_radians());
            let scale_vec = Vec3::splat(uniform_scale);
            let scale = doodad_catalog
                .get(id)
                .map(|def| doodad_final_render_scale(def, scale_vec))
                .unwrap_or(scale_vec);
            Transform {
                translation: world_position_to_render_global(position, layout, vertical_scale),
                rotation,
                scale,
            }
        }
        DefinitionId::Building(id) => {
            let rotation = Quat::from_rotation_y(yaw_deg.to_radians());
            let grounded = ground_and_quantize_building_anchor(world, position).unwrap_or(position);
            let fixed_scale = FixedScale::from_f32(uniform_scale).unwrap_or(FixedScale::ONE);
            let placement =
                BuildingPlacement::new(grounded, rotation).with_uniform_scale(fixed_scale);
            let definition = building_catalog.get(id);
            if matches!(
                visual,
                DevPlacementModelVisual::BuildingScene {
                    uses_model_child: true,
                    ..
                }
            ) && definition.is_some() {
                building_anchor_render_transform(
                    definition.unwrap(),
                    &placement,
                    layout,
                    vertical_scale,
                )
            } else if let Some(definition) = definition {
                building_model_render_transform(definition, &placement, layout, vertical_scale)
            } else {
                Transform::from_translation(
                    world_position_to_render_global(grounded, layout, vertical_scale),
                )
            }
        }
        DefinitionId::Item(item_id) => {
            let definition = item_catalog.get(item_id);
            let translation = world_position_to_render_global(position, layout, vertical_scale);
            let y = if matches!(visual, DevPlacementModelVisual::ItemFallbackSphere) {
                translation.y + pile_settings.fallback_sphere_radius
            } else {
                translation.y
            };
            let rotation = item_pile_visual_rotation(Quat::IDENTITY, definition);
            Transform {
                translation: Vec3::new(translation.x, y, translation.z),
                rotation,
                scale: Vec3::ONE,
            }
        }
        DefinitionId::InventoryProfile(_) => Transform::IDENTITY,
    }
}

fn spawn_ghost_root(
    commands: &mut Commands,
    visual: &DevPlacementModelVisual,
    slot: usize,
    transform: Transform,
    asset_server: &AssetServer,
    unit_scenes: &mut UnitSceneAssets,
    doodad_scenes: &mut DoodadSceneAssets,
    building_scenes: &mut BuildingSceneAssets,
    item_scenes: &mut ItemSceneAssets,
    building_catalog: &BuildingCatalog,
    item_catalog: &ItemCatalog,
    pile_settings: &ItemPilePresentationSettings,
    pile_fallback: &mut ItemPileFallbackAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    definition: &DefinitionId,
    uniform_scale: f32,
) -> Option<Entity> {
    let marker = (
        DevPlacementModelGhost { slot },
        DevPlacementModelGhostRoot,
        transform,
        Visibility::default(),
        Name::new(format!("DevPlacementModelGhost[{slot}]")),
    );

    match visual {
        DevPlacementModelVisual::UnitScene { render_key } => {
            let Some(scene) = unit_scenes.ensure_scene_for_render_key(render_key, asset_server)
            else {
                return None;
            };
            if !scene_loaded(asset_server, &scene) {
                return None;
            }
            Some(commands.spawn((marker, SceneRoot(scene))).id())
        }
        DevPlacementModelVisual::DoodadScene {
            definition_id,
            render_key,
        } => {
            let Some(scene) = doodad_scenes.ensure_scene(
                definition_id,
                &DoodadRenderKey::reserved(render_key),
                asset_server,
            ) else {
                return None;
            };
            if !scene_loaded(asset_server, &scene) {
                return None;
            }
            Some(commands.spawn((marker, SceneRoot(scene))).id())
        }
        DevPlacementModelVisual::BuildingScene {
            render_key,
            uses_model_child,
            definition_id,
        } => {
            let Some(scene) = building_scenes.ensure_scene(render_key, asset_server) else {
                return None;
            };
            if !scene_loaded(asset_server, &scene) {
                return None;
            };
            if *uses_model_child {
                let definition = building_catalog.get(definition_id)?;
                let correction =
                    building_model_child_local_transform(definition, uniform_scale);
                Some(
                    commands
                        .spawn(marker)
                        .with_children(|parent| {
                            parent.spawn((SceneRoot(scene), correction));
                        })
                        .id(),
                )
            } else {
                Some(commands.spawn((marker, SceneRoot(scene))).id())
            }
        }
        DevPlacementModelVisual::ItemScene { render_key, .. } => {
            let Some(scene) = item_scenes.ensure_scene(render_key, asset_server) else {
                return None;
            };
            if !scene_loaded(asset_server, &scene) {
                return None;
            }
            Some(commands.spawn((marker, SceneRoot(scene))).id())
        }
        DevPlacementModelVisual::ItemFallbackSphere => {
            let DefinitionId::Item(item_id) = definition else {
                return None;
            };
            let definition = item_catalog.get(item_id);
            let mesh = pile_fallback.mesh(meshes, pile_settings);
            let material = pile_fallback.material_for_definition(
                materials,
                pile_settings,
                definition,
                definition.is_some_and(|def| def.unique_instance_required),
            );
            Some(
                commands
                    .spawn((marker, Mesh3d(mesh), MeshMaterial3d(material)))
                    .id(),
            )
        }
    }
}

fn scene_loaded(asset_server: &AssetServer, scene: &Handle<Scene>) -> bool {
    matches!(asset_server.get_load_state(scene), Some(LoadState::Loaded))
}

fn tint_pending_previews(
    commands: &mut Commands,
    pending: &Query<Entity, With<DevPlacementModelTintPending>>,
    children: &Query<&Children>,
    mesh_materials: &Query<&MeshMaterial3d<StandardMaterial>>,
    originals: &Query<&OriginalBuildingMaterial>,
    materials: &mut Assets<StandardMaterial>,
) {
    for entity in pending.iter() {
        tint_preview_hierarchy(commands, entity, children, mesh_materials, originals, materials);
        commands.entity(entity).remove::<DevPlacementModelTintPending>();
    }
}

fn tint_preview_hierarchy(
    commands: &mut Commands,
    entity: Entity,
    children: &Query<&Children>,
    mesh_materials: &Query<&MeshMaterial3d<StandardMaterial>>,
    originals: &Query<&OriginalBuildingMaterial>,
    materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(mesh_material) = mesh_materials.get(entity) {
        let original_handle = if let Ok(original) = originals.get(entity) {
            original.handle.clone()
        } else {
            let handle = mesh_material.0.clone();
            commands
                .entity(entity)
                .insert(OriginalBuildingMaterial::new(handle.clone()));
            handle
        };
        let mut cloned = materials.get(&original_handle).cloned().unwrap_or_default();
        apply_dev_preview_material_style(&mut cloned);
        let tinted = materials.add(cloned);
        commands.entity(entity).insert(MeshMaterial3d(tinted));
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            tint_preview_hierarchy(commands, child, children, mesh_materials, originals, materials);
        }
    }
}

fn apply_dev_preview_material_style(material: &mut StandardMaterial) {
    material.unlit = true;
    material.base_color = Color::srgba(0.55, 0.82, 1.0, 0.42);
    material.alpha_mode = AlphaMode::Blend;
}
