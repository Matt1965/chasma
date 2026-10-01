//! Portrait capture queue and render-to-texture pipeline.

use bevy::asset::LoadState;
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::camera::render_layers::PORTRAIT_RENDER_LAYER;
use crate::units::components::UnitRenderMetadata;
use crate::units::presentation::UnitPresentationAppearance;
use crate::units::UnitSceneAssets;
use crate::world::{
    AppearanceProfileCatalog, EquipmentVisualCatalog, ItemCatalog, UnitCatalog, UnitId,
    UnitAppearance, UnitDefinition, UnitRecord, WorldData, effective_unit_render_key_str,
    resolve_canonical_default_appearance, unit_visual_rotation, unit_visual_scale,
};

use super::cache::{PortraitAppearanceSignature, PortraitCaptureRequest, UnitPortraitCache};
use super::components::{
    UnitPortraitActor, UnitPortraitCamera, UnitPortraitFraming, UnitPortraitSceneRoot,
    UnitPortraitStageRoot,
};
use super::equipment::{UnitPortraitEquipmentIndex, clear_portrait_equipment_for_actor};
use super::diagnostics::{
    portrait_diagnostic_step, portrait_diagnostics_block_cache,
};
use super::framing::{
    PORTRAIT_UNIT_YAW, measure_actor_bounds, portrait_camera_transform,
    portrait_framing_from_bounds, portrait_generous_framing_from_bounds,
};
use super::signature::portrait_signature_for_unit;
use super::studio::new_portrait_render_target_image;
use super::studio_images::UnitPortraitStudioImages;

const PORTRAIT_WARMUP_FRAMES: u32 = 4;
const PORTRAIT_RENDER_SETTLE_FRAMES: u32 = 3;

#[derive(SystemParam)]
pub struct PortraitCaptureWorldParams<'w> {
    pub demand: Res<'w, UnitPortraitUiDemand>,
    pub studio: Res<'w, UnitPortraitStudioImages>,
    pub cache: ResMut<'w, UnitPortraitCache>,
    pub capture: ResMut<'w, UnitPortraitCaptureState>,
    pub images: ResMut<'w, Assets<Image>>,
    pub world: Res<'w, WorldData>,
    pub unit_catalog: Res<'w, UnitCatalog>,
    pub appearance_profiles: Res<'w, AppearanceProfileCatalog>,
    pub scene_assets: ResMut<'w, UnitSceneAssets>,
    pub asset_server: Res<'w, AssetServer>,
    pub equipment_index: ResMut<'w, UnitPortraitEquipmentIndex>,
}

#[derive(SystemParam)]
pub struct PortraitCaptureActorQueries<'w, 's> {
    pub stage_roots: Query<'w, 's, Entity, With<UnitPortraitStageRoot>>,
    pub cameras: Query<'w, 's, &'static mut RenderTarget, With<UnitPortraitCamera>>,
    pub actors: Query<'w, 's, Entity, With<UnitPortraitActor>>,
    pub actor_roots: Query<'w, 's, Entity, With<UnitPortraitSceneRoot>>,
    pub framing_ready: Query<'w, 's, &'static UnitPortraitFraming>,
    pub children: Query<'w, 's, &'static Children>,
    pub mesh3d: Query<'w, 's, &'static Mesh3d>,
    pub equipment_visuals: Query<'w, 's, Entity, With<crate::units::equipment_presentation::UnitEquipmentVisual>>,
}

#[derive(Resource, Debug, Default)]
pub struct UnitPortraitCaptureState {
    pub active_request: Option<PortraitCaptureRequest>,
    pub actor_entity: Option<Entity>,
    pub target_image: Option<Handle<Image>>,
    pub warmup_frames_remaining: u32,
    pub render_settle_frames: u32,
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
    if portrait_diagnostics_block_cache(portrait_diagnostic_step()) {
        return;
    }
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
    let step = portrait_diagnostic_step();
    for (entity, metadata) in &actors {
        if let Some((center, height)) =
            measure_actor_bounds(entity, &children, &mesh3d, &meshes, &global_transforms)
        {
            let framing = if step == 5 {
                portrait_generous_framing_from_bounds(center, height)
            } else if let Some(definition) = unit_catalog.get(&metadata.definition_id) {
                portrait_framing_from_bounds(center, height, definition)
            } else {
                continue;
            };
            commands.entity(entity).insert(framing);
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
    mut world_params: PortraitCaptureWorldParams,
    mut queries: PortraitCaptureActorQueries,
) {
    let step = portrait_diagnostic_step();
    if portrait_diagnostics_block_cache(step) {
        if step >= 5 {
            drive_diagnostic_live_unit(
                &mut commands,
                &world_params.demand,
                &world_params.studio,
                &mut world_params.capture,
                &world_params.world,
                &world_params.unit_catalog,
                &world_params.appearance_profiles,
                &mut world_params.scene_assets,
                &world_params.asset_server,
                &queries.stage_roots,
                &mut queries.cameras,
                &queries.children,
                &queries.mesh3d,
                &mut world_params.equipment_index,
                &queries.equipment_visuals,
                step,
            );
        }
        return;
    }

    if world_params.capture.active_request.is_none() {
        begin_next_capture(
            &mut commands,
            &mut world_params.cache,
            &mut world_params.capture,
            &mut world_params.images,
            &world_params.world,
            &world_params.unit_catalog,
            &world_params.appearance_profiles,
            &mut world_params.scene_assets,
            &world_params.asset_server,
            &queries.stage_roots,
            &mut queries.cameras,
        );
        return;
    }

    let request = world_params.capture.active_request.unwrap();
    if world_params.world.get_unit(request.unit_id).is_none() {
        finish_capture(
            &mut commands,
            &mut world_params.capture,
            &mut world_params.equipment_index,
            &queries.equipment_visuals,
            false,
            &mut world_params.cache,
        );
        return;
    }

    let Some(actor_entity) = world_params.capture.actor_entity else {
        return;
    };
    if queries.actors.get(actor_entity).is_err() || queries.actor_roots.get(actor_entity).is_err() {
        finish_capture(
            &mut commands,
            &mut world_params.capture,
            &mut world_params.equipment_index,
            &queries.equipment_visuals,
            false,
            &mut world_params.cache,
        );
        return;
    }

    if queries.framing_ready.get(actor_entity).is_err() {
        return;
    }

    if count_actor_meshes(actor_entity, &queries.children, &queries.mesh3d) == 0 {
        return;
    }

    if world_params.capture.warmup_frames_remaining > 0 {
        world_params.capture.warmup_frames_remaining -= 1;
        if world_params.capture.warmup_frames_remaining == 0 {
            world_params.capture.render_settle_frames = PORTRAIT_RENDER_SETTLE_FRAMES;
        }
        return;
    }

    if world_params.capture.render_settle_frames > 0 {
        world_params.capture.render_settle_frames -= 1;
        if world_params.capture.render_settle_frames > 0 {
            return;
        }
    }

    let image = world_params.capture.target_image.clone().unwrap();
    let committed = world_params.cache.commit_capture(
        request.unit_id,
        request.generation,
        request.signature,
        image,
    );
    finish_capture(
        &mut commands,
        &mut world_params.capture,
        &mut world_params.equipment_index,
        &queries.equipment_visuals,
        committed,
        &mut world_params.cache,
    );
}

fn portrait_appearance_for_unit(
    unit: &UnitRecord,
    definition: &UnitDefinition,
    profiles: &AppearanceProfileCatalog,
) -> Option<UnitAppearance> {
    if let Some(appearance) = unit.appearance.clone() {
        return Some(appearance);
    }
    resolve_canonical_default_appearance(definition, profiles).ok()
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
        cache.coalesce_queue(request.unit_id, request.signature, request.generation);
        return;
    };
    let render_key_str = match effective_unit_render_key_str(
        unit,
        definition,
        appearance_profiles,
    ) {
        Ok(key) => key,
        Err(_) => {
            cache.coalesce_queue(request.unit_id, request.signature, request.generation);
            return;
        }
    };
    let Some(scene) = scene_assets.scene_for_render_key(&render_key_str).cloned() else {
        cache.coalesce_queue(request.unit_id, request.signature, request.generation);
        return;
    };
    if !matches!(asset_server.get_load_state(&scene), Some(LoadState::Loaded)) {
        cache.coalesce_queue(request.unit_id, request.signature, request.generation);
        return;
    }

    let handle = new_portrait_render_target_image(images);
    for mut target in cameras.iter_mut() {
        *target = RenderTarget::Image(handle.clone().into());
    }

    let appearance = portrait_appearance_for_unit(unit, definition, appearance_profiles);
    let height_scale = appearance.as_ref().map(|value| value.height_scale).unwrap_or(1.0);
    let visual_scale = unit_visual_scale(definition, height_scale);
    let facing = unit_visual_rotation(definition, Quat::from_rotation_y(PORTRAIT_UNIT_YAW));

    let mut actor = commands.spawn((
        UnitPortraitActor {
            unit_id: request.unit_id,
            request_generation: request.generation,
        },
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
    ));
    if let Some(appearance) = appearance {
        actor.insert(UnitPresentationAppearance { appearance });
    }
    let actor = actor.id();
    commands.entity(parent).add_child(actor);

    capture.active_request = Some(request);
    capture.actor_entity = Some(actor);
    capture.target_image = Some(handle);
    capture.warmup_frames_remaining = PORTRAIT_WARMUP_FRAMES;
    capture.render_settle_frames = 0;
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
    capture.render_settle_frames = 0;
}

fn count_actor_meshes(root: Entity, children: &Query<&Children>, mesh3d: &Query<&Mesh3d>) -> u32 {
    let mut count = 0u32;
    count_meshes_recursive(root, children, mesh3d, &mut count);
    count
}

fn count_meshes_recursive(
    entity: Entity,
    children: &Query<&Children>,
    mesh3d: &Query<&Mesh3d>,
    count: &mut u32,
) {
    if mesh3d.get(entity).is_ok() {
        *count += 1;
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            count_meshes_recursive(child, children, mesh3d, count);
        }
    }
}

fn drive_diagnostic_live_unit(
    commands: &mut Commands,
    demand: &UnitPortraitUiDemand,
    studio: &UnitPortraitStudioImages,
    capture: &mut UnitPortraitCaptureState,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    scene_assets: &mut UnitSceneAssets,
    asset_server: &AssetServer,
    stage_roots: &Query<Entity, With<UnitPortraitStageRoot>>,
    cameras: &mut Query<&mut RenderTarget, With<UnitPortraitCamera>>,
    children: &Query<&Children>,
    mesh3d: &Query<&Mesh3d>,
    equipment_index: &mut UnitPortraitEquipmentIndex,
    equipment_visuals: &Query<Entity, With<crate::units::equipment_presentation::UnitEquipmentVisual>>,
    step: u8,
) {
    let Some(unit_id) = demand.primary_unit else {
        if let Some(actor) = capture.actor_entity {
            clear_portrait_equipment_for_actor(commands, actor, equipment_index, equipment_visuals);
            commands.entity(actor).despawn();
            capture.actor_entity = None;
        }
        return;
    };
    for mut target in cameras.iter_mut() {
        *target = RenderTarget::Image(studio.live_target.clone().into());
    }
    if let Some(actor) = capture.actor_entity {
        if capture
            .active_request
            .is_some_and(|request| request.unit_id == unit_id)
        {
            let _ = count_actor_meshes(actor, children, mesh3d);
            return;
        }
        clear_portrait_equipment_for_actor(commands, actor, equipment_index, equipment_visuals);
        commands.entity(actor).despawn();
        capture.actor_entity = None;
    }
    let Some(parent) = stage_roots.iter().next() else {
        return;
    };
    let Some(unit) = world.get_unit(unit_id) else {
        return;
    };
    let Some(definition) = unit_catalog.get(&unit.definition_id) else {
        return;
    };
    let render_key_str = match effective_unit_render_key_str(
        unit,
        definition,
        appearance_profiles,
    ) {
        Ok(key) => key,
        Err(_) => return,
    };
    let Some(scene) = scene_assets.scene_for_render_key(&render_key_str).cloned() else {
        return;
    };
    if !matches!(asset_server.get_load_state(&scene), Some(LoadState::Loaded)) {
        return;
    }
    let appearance = portrait_appearance_for_unit(unit, definition, appearance_profiles);
    let height_scale = appearance.as_ref().map(|value| value.height_scale).unwrap_or(1.0);
    let visual_scale = unit_visual_scale(definition, height_scale);
    let facing = unit_visual_rotation(definition, Quat::from_rotation_y(PORTRAIT_UNIT_YAW));
    let mut actor_cmd = commands.spawn((
        UnitPortraitActor {
            unit_id,
            request_generation: 0,
        },
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
    ));
    if let Some(appearance) = appearance {
        actor_cmd.insert(UnitPresentationAppearance { appearance });
    }
    let actor = actor_cmd.id();
    commands.entity(parent).add_child(actor);
    capture.actor_entity = Some(actor);
    capture.active_request = Some(PortraitCaptureRequest {
        unit_id,
        signature: PortraitAppearanceSignature {
            unit_id,
            digest: step as u64,
        },
        generation: 0,
    });
    capture.target_image = Some(studio.live_target.clone());
    capture.warmup_frames_remaining = 0;
    capture.render_settle_frames = 1;
}

#[cfg(test)]
mod tests {
    use super::super::studio_images::new_portrait_render_target;
    use super::super::studio::PORTRAIT_TEXTURE_SIZE;

    #[test]
    fn portrait_texture_size_is_256() {
        assert_eq!(PORTRAIT_TEXTURE_SIZE, 256);
    }
}
