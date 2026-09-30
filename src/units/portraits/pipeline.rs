//! Portrait capture queue and render-to-texture pipeline.

use bevy::asset::LoadState;
use bevy::camera::RenderTarget;
use bevy::prelude::*;

use crate::camera::render_layers::PORTRAIT_RENDER_LAYER;
use crate::units::components::UnitRenderMetadata;
use crate::units::presentation::UnitPresentationAppearance;
use crate::units::UnitSceneAssets;
use crate::world::{
    AppearanceProfileCatalog, EquipmentVisualCatalog, ItemCatalog, UnitCatalog, UnitId,
    WorldData, effective_render_key_for_appearance, unit_visual_rotation, unit_visual_scale,
};

use super::cache::{PortraitCaptureRequest, UnitPortraitCache};
use super::components::{
    UnitPortraitActor, UnitPortraitCamera, UnitPortraitFraming, UnitPortraitSceneRoot,
    UnitPortraitStageRoot,
};
use super::equipment::{UnitPortraitEquipmentIndex, clear_portrait_equipment_for_actor};
use super::framing::{
    PORTRAIT_UNIT_YAW, measure_actor_bounds, portrait_camera_transform, portrait_framing_from_bounds,
};
use super::signature::portrait_signature_for_unit;
use super::studio::new_portrait_render_target;

const PORTRAIT_WARMUP_FRAMES: u32 = 2;
const PORTRAIT_CAPTURE_FRAMES: u32 = 1;

#[derive(Resource, Debug, Default)]
pub struct UnitPortraitCaptureState {
    pub active_request: Option<PortraitCaptureRequest>,
    pub actor_entity: Option<Entity>,
    pub target_image: Option<Handle<Image>>,
    pub warmup_frames_remaining: u32,
    pub capture_frames_remaining: u32,
}

/// Primary units that need fresh portraits (HUD writes here each frame).
#[derive(Resource, Debug, Default)]
pub struct UnitPortraitUiDemand {
    pub primary_unit: Option<UnitId>,
}

pub fn maintain_portrait_cache_requests(
    demand: Res<UnitPortraitUiDemand>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    items: Res<ItemCatalog>,
    visuals: Res<EquipmentVisualCatalog>,
    mut cache: ResMut<UnitPortraitCache>,
) {
    cache.max_entries = UnitPortraitCache::DEFAULT_CAPACITY;
    cache.prune_missing_units(|id| world.get_unit(id).is_some());
    let Some(unit_id) = demand.primary_unit else {
        return;
    };
    let Some(unit) = world.get_unit(unit_id) else {
        cache.remove_unit(unit_id);
        return;
    };
    let Some(signature) = portrait_signature_for_unit(
        &world,
        unit,
        &unit_catalog,
        &appearance_profiles,
        &items,
        &visuals,
    ) else {
        return;
    };
    if !cache.entry_matches(unit_id, &signature) {
        cache.request_capture(unit_id, signature);
    }
}

pub fn update_portrait_actor_framing(
    unit_catalog: Res<UnitCatalog>,
    mut commands: Commands,
    actors: Query<
        (Entity, &UnitRenderMetadata),
        (With<UnitPortraitSceneRoot>, Without<UnitPortraitFraming>),
    >,
    children: Query<&Children>,
    mesh3d: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
    global_transforms: Query<&GlobalTransform>,
) {
    for (entity, metadata) in &actors {
        if let Some((center, height)) =
            measure_actor_bounds(entity, &children, &mesh3d, &meshes, &global_transforms)
        {
            if let Some(definition) = unit_catalog.get(&metadata.definition_id) {
                commands
                    .entity(entity)
                    .insert(portrait_framing_from_bounds(center, height, definition));
            }
        }
    }
}

pub fn sync_portrait_capture_camera(
    capture: Res<UnitPortraitCaptureState>,
    actors: Query<&UnitPortraitFraming, With<UnitPortraitSceneRoot>>,
    mut cameras: Query<&mut Transform, With<UnitPortraitCamera>>,
) {
    if capture.active_request.is_none() {
        return;
    }
    let Some(actor) = capture.actor_entity else {
        return;
    };
    let Ok(framing) = actors.get(actor) else {
        return;
    };
    let transform = portrait_camera_transform(framing);
    for mut camera_transform in &mut cameras {
        *camera_transform = transform;
    }
}

pub fn drive_portrait_capture_pipeline(
    mut commands: Commands,
    mut cache: ResMut<UnitPortraitCache>,
    mut capture: ResMut<UnitPortraitCaptureState>,
    mut images: ResMut<Assets<Image>>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    mut scene_assets: ResMut<UnitSceneAssets>,
    asset_server: Res<AssetServer>,
    stage_roots: Query<Entity, With<UnitPortraitStageRoot>>,
    mut cameras: Query<&mut RenderTarget, With<UnitPortraitCamera>>,
    actors: Query<Entity, With<UnitPortraitActor>>,
    actor_roots: Query<Entity, With<UnitPortraitSceneRoot>>,
    framing_ready: Query<&UnitPortraitFraming>,
    equipment_visuals: Query<Entity, With<crate::units::equipment_presentation::UnitEquipmentVisual>>,
    mut equipment_index: ResMut<UnitPortraitEquipmentIndex>,
) {
    if capture.active_request.is_none() {
        begin_next_capture(
            &mut commands,
            &mut cache,
            &mut capture,
            &mut images,
            &world,
            &unit_catalog,
            &appearance_profiles,
            &mut scene_assets,
            &asset_server,
            &stage_roots,
            &mut cameras,
        );
        return;
    }

    let request = capture.active_request.unwrap();
    if world.get_unit(request.unit_id).is_none() {
        finish_capture(
            &mut commands,
            &mut capture,
            &mut equipment_index,
            &equipment_visuals,
            false,
            &mut cache,
        );
        return;
    }

    let Some(actor_entity) = capture.actor_entity else {
        return;
    };
    if actors.get(actor_entity).is_err() || actor_roots.get(actor_entity).is_err() {
        finish_capture(
            &mut commands,
            &mut capture,
            &mut equipment_index,
            &equipment_visuals,
            false,
            &mut cache,
        );
        return;
    }

    if framing_ready.get(actor_entity).is_err() {
        return;
    }

    if capture.warmup_frames_remaining > 0 {
        capture.warmup_frames_remaining -= 1;
        return;
    }

    if capture.capture_frames_remaining > 0 {
        capture.capture_frames_remaining -= 1;
        if capture.capture_frames_remaining == 0 {
            let image = capture.target_image.clone().unwrap();
            let committed = cache.commit_capture(
                request.unit_id,
                request.generation,
                request.signature,
                image,
            );
            finish_capture(
                &mut commands,
                &mut capture,
                &mut equipment_index,
                &equipment_visuals,
                committed,
                &mut cache,
            );
        }
    }
}

fn begin_next_capture(
    commands: &mut Commands,
    cache: &mut UnitPortraitCache,
    capture: &mut UnitPortraitCaptureState,
    images: &mut Assets<Image>,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    scene_assets: &mut UnitSceneAssets,
    asset_server: &AssetServer,
    stage_roots: &Query<Entity, With<UnitPortraitStageRoot>>,
    cameras: &mut Query<&mut RenderTarget, With<UnitPortraitCamera>>,
) {
    let Some(request) = cache.pop_next_request() else {
        return;
    };
    let Some(parent) = stage_roots.iter().next() else {
        cache.coalesce_queue(request.unit_id, request.signature, request.generation);
        return;
    };
    let Some(unit) = world.get_unit(request.unit_id) else {
        return;
    };
    let Some(definition) = unit_catalog.get(&unit.definition_id) else {
        return;
    };
    let Some(appearance) = unit.appearance.clone() else {
        return;
    };
    let render_key = match effective_render_key_for_appearance(&appearance, appearance_profiles) {
        Ok(key) => key,
        Err(_) => return,
    };
    let render_key_str = render_key.0.as_deref().unwrap_or("");
    let Some(scene) = scene_assets.scene_for_render_key(render_key_str).cloned() else {
        cache.coalesce_queue(request.unit_id, request.signature, request.generation);
        return;
    };
    if !matches!(asset_server.get_load_state(&scene), Some(LoadState::Loaded)) {
        cache.coalesce_queue(request.unit_id, request.signature, request.generation);
        return;
    }

    let handle = new_portrait_render_target(images);
    for mut target in cameras.iter_mut() {
        *target = RenderTarget::Image(handle.clone().into());
    }

    let visual_scale = unit_visual_scale(definition, appearance.height_scale);
    let facing = unit_visual_rotation(definition, Quat::from_rotation_y(PORTRAIT_UNIT_YAW));
    let actor = commands
        .spawn((
            UnitPortraitActor {
                unit_id: request.unit_id,
                request_generation: request.generation,
            },
            UnitPresentationAppearance { appearance },
            UnitRenderMetadata {
                definition_id: unit.definition_id.clone(),
            },
            UnitPortraitSceneRoot,
            SceneRoot(scene),
            Transform {
                translation: Vec3::ZERO,
                rotation: facing,
                scale: visual_scale,
            },
            Visibility::default(),
            PORTRAIT_RENDER_LAYER,
        ))
        .id();
    commands.entity(parent).add_child(actor);

    capture.active_request = Some(request);
    capture.actor_entity = Some(actor);
    capture.target_image = Some(handle);
    capture.warmup_frames_remaining = PORTRAIT_WARMUP_FRAMES;
    capture.capture_frames_remaining = PORTRAIT_CAPTURE_FRAMES;
}

fn finish_capture(
    commands: &mut Commands,
    capture: &mut UnitPortraitCaptureState,
    equipment_index: &mut UnitPortraitEquipmentIndex,
    equipment_visuals: &Query<Entity, With<crate::units::equipment_presentation::UnitEquipmentVisual>>,
    committed: bool,
    cache: &mut UnitPortraitCache,
) {
    if let Some(actor) = capture.actor_entity {
        clear_portrait_equipment_for_actor(commands, actor, equipment_index, equipment_visuals);
        if commands.get_entity(actor).is_ok() {
            commands.entity(actor).despawn();
        }
    }
    if !committed {
        if let Some(request) = capture.active_request {
            cache.coalesce_queue(request.unit_id, request.signature, request.generation);
        }
    }
    capture.active_request = None;
    capture.actor_entity = None;
    capture.target_image = None;
    capture.warmup_frames_remaining = 0;
    capture.capture_frames_remaining = 0;
}

// coalesce_queue is on cache - made pub(crate) - need to expose for pipeline

#[cfg(test)]
mod tests {
    use super::super::studio::PORTRAIT_TEXTURE_SIZE;

    #[test]
    fn portrait_texture_size_is_256() {
        assert_eq!(PORTRAIT_TEXTURE_SIZE, 256);
    }
}
