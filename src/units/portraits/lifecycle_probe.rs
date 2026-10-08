//! Runtime portrait lifecycle probe (dev). One build; change stage at runtime.
//!
//! ## Controls
//!
//! | Key | Action |
//! |-----|--------|
//! | **F10** | Advance stage `0 → 1 → … → 7 → 0` |
//! | **F9** | Log image ids, generation, camera, mesh, cache vs UI |
//!
//! Uses production [`new_portrait_render_target_image`], left HUD [`sync_selected_unit_portrait_ui`],
//! and [`UnitPortraitCache::commit_capture`] (not `live_target` alone).
//!
//! ## Stages
//!
//! | Stage | Purpose |
//! |-------|---------|
//! | **0** | Production. Clears probe mesh and **removes probe cache entries** so a fresh capture is requested. |
//! | **1** | Green probe mesh + camera **ON** → HUD binds `probe_image` directly. Failure here is an **unresolved** UI / camera / geometry boundary (not a proven UI-only bug). |
//! | **2** | **Publish** visible mesh into cache for selected unit (mesh + camera stay **ON**). |
//! | **3** | **Comparison A (unsafe):** remove mesh, camera stays **ON** (production `finish_capture` order). |
//! | **4** | **Comparison A:** camera **OFF** (mesh already gone). If green vanished at 3–4, A shows post-publish clear risk. |
//! | **5** | **Comparison B (fresh):** new `probe_image`, respawn mesh, **publish** again (new generation). |
//! | **6** | **Comparison B (safe):** camera **OFF** before mesh removal. |
//! | **7** | **Comparison B:** remove mesh. If B keeps green vs A, cleanup-order defect is demonstrated. |
//!
//! Stage 3 (old “safe” camera-off with mesh) is **not** required to fail for production’s bug to exist.
//!
//! ## Bevy 0.18 order (production `finish_capture`)
//!
//! Update: camera may stay **on** while `active_request` was set earlier.  
//! PostUpdate: `commit_capture` → `finish_capture` (despawn actor; camera **not** turned off).  
//! Render: GPU may draw clear/backdrop into the published `Image` with no actor.

use std::collections::HashSet;

use bevy::camera::{Camera, RenderTarget};
use bevy::prelude::*;

use crate::camera::render_layers::PORTRAIT_RENDER_LAYER;
use crate::world::{
    AppearanceProfileCatalog, EquipmentVisualCatalog, ItemCatalog, UnitCatalog, UnitId, WorldData,
};

use super::cache::UnitPortraitCache;
use super::components::{UnitPortraitActor, UnitPortraitCamera, UnitPortraitStageRoot};
use super::pipeline::{UnitPortraitCaptureState, UnitPortraitUiDemand};
use super::signature::portrait_signature_for_unit;
use super::studio::{PORTRAIT_STAGE_OFFSET, new_portrait_render_target_image};

pub const PORTRAIT_LIFECYCLE_STAGE_COUNT: u8 = 8;

#[derive(Component, Debug)]
pub(crate) struct UnitPortraitLifecycleProbeMesh;

#[derive(Resource, Debug)]
pub struct PortraitLifecycleProbe {
    pub stage: u8,
    pub probe_image: Option<Handle<Image>>,
    pub probe_mesh: Option<Entity>,
    pub probe_unit: Option<UnitId>,
    pub published_generation: u64,
    pub published_image_id: Option<AssetId<Image>>,
    /// Units whose cache rows were created by this probe (stripped on return to stage 0).
    pub probe_cache_units: HashSet<UnitId>,
    pub on_stage_entered: bool,
    pub pending_log: Option<String>,
    pub last_logged_stage: u8,
}

impl Default for PortraitLifecycleProbe {
    fn default() -> Self {
        Self {
            stage: 0,
            probe_image: None,
            probe_mesh: None,
            probe_unit: None,
            published_generation: 0,
            published_image_id: None,
            probe_cache_units: HashSet::new(),
            on_stage_entered: false,
            pending_log: None,
            last_logged_stage: 255,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct PortraitLifecycleTrace {
    pub stage: u8,
    pub camera_active: bool,
    pub camera_target_id: Option<AssetId<Image>>,
    pub ui_image_id: Option<AssetId<Image>>,
    pub cache_image_id: Option<AssetId<Image>>,
    pub probe_image_id: Option<AssetId<Image>>,
    pub published_generation: u64,
    pub capture_active: bool,
    pub capture_target_id: Option<AssetId<Image>>,
    pub actor_entity_alive: bool,
    pub probe_mesh_alive: bool,
}

pub fn portrait_lifecycle_blocks_production(probe: &PortraitLifecycleProbe) -> bool {
    probe.stage >= 1 && probe.stage <= 7
}

pub fn lifecycle_probe_hud_image(
    probe: &PortraitLifecycleProbe,
    cache: &UnitPortraitCache,
    primary: Option<UnitId>,
) -> Option<Handle<Image>> {
    if probe.stage == 0 {
        return None;
    }
    if probe.stage == 1 {
        return probe.probe_image.clone();
    }
    primary
        .and_then(|id| cache.image_for_unit(id).cloned())
        .or_else(|| probe.probe_image.clone())
}

#[cfg(feature = "dev")]
pub fn portrait_lifecycle_probe_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut probe: ResMut<PortraitLifecycleProbe>,
) {
    if keyboard.just_pressed(KeyCode::F9) {
        probe.pending_log = Some("manual F9 snapshot".to_string());
    }
    if keyboard.just_pressed(KeyCode::F10) {
        let next = (probe.stage + 1) % PORTRAIT_LIFECYCLE_STAGE_COUNT;
        probe.stage = next;
        probe.on_stage_entered = true;
        probe.pending_log = Some(format!("F10 -> lifecycle stage {}", probe.stage));
    }
}

#[cfg(not(feature = "dev"))]
pub fn portrait_lifecycle_probe_input(_keyboard: Res<ButtonInput<KeyCode>>) {}

pub fn sync_portrait_lifecycle_camera_override(
    probe: Res<PortraitLifecycleProbe>,
    mut cameras: Query<&mut Camera, With<UnitPortraitCamera>>,
) {
    if probe.stage == 0 {
        return;
    }
    let active = matches!(probe.stage, 1 | 2 | 3 | 5);
    for mut camera in &mut cameras {
        camera.is_active = active;
    }
}

fn generous_probe_camera_transform() -> Transform {
    let eye = PORTRAIT_STAGE_OFFSET + Vec3::new(0.0, 1.05, 1.75);
    let focus = PORTRAIT_STAGE_OFFSET + Vec3::new(0.0, 1.05, 0.0);
    Transform::from_translation(eye).looking_at(focus, Vec3::Y)
}

fn cleanup_probe_state(
    commands: &mut Commands,
    probe: &mut PortraitLifecycleProbe,
    cache: &mut UnitPortraitCache,
    probe_meshes: &Query<Entity, With<UnitPortraitLifecycleProbeMesh>>,
) {
    for entity in probe_meshes.iter() {
        commands.entity(entity).despawn();
    }
    for unit_id in probe.probe_cache_units.iter().copied().collect::<Vec<_>>() {
        cache.remove_unit(unit_id);
    }
    probe.probe_cache_units.clear();
    probe.probe_mesh = None;
    probe.probe_image = None;
    probe.probe_unit = None;
    probe.published_generation = 0;
    probe.published_image_id = None;
    probe.pending_log = Some(
        "lifecycle stage 0: probe cache entries removed; production may request fresh capture"
            .to_string(),
    );
}

fn despawn_probe_mesh(
    commands: &mut Commands,
    probe: &mut PortraitLifecycleProbe,
    probe_meshes: &Query<Entity, With<UnitPortraitLifecycleProbeMesh>>,
) {
    if let Some(entity) = probe.probe_mesh {
        if probe_meshes.get(entity).is_ok() {
            commands.entity(entity).despawn();
        }
    }
    probe.probe_mesh = None;
}

fn spawn_probe_mesh(
    commands: &mut Commands,
    probe: &mut PortraitLifecycleProbe,
    stage_roots: &Query<Entity, With<UnitPortraitStageRoot>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(stage_root) = stage_roots.iter().next() else {
        return false;
    };
    let mesh = meshes.add(Cuboid::new(0.7, 0.9, 0.35));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.95, 0.35),
        unlit: true,
        ..default()
    });
    let entity = commands
        .spawn((
            UnitPortraitLifecycleProbeMesh,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(Vec3::new(0.0, 1.05, 0.0)),
            Visibility::default(),
            PORTRAIT_RENDER_LAYER,
        ))
        .id();
    commands.entity(stage_root).add_child(entity);
    probe.probe_mesh = Some(entity);
    true
}

fn publish_probe_to_cache(
    probe: &mut PortraitLifecycleProbe,
    cache: &mut UnitPortraitCache,
    unit_id: UnitId,
    probe_image: Handle<Image>,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
) -> Option<String> {
    let Some(unit) = world.get_unit(unit_id) else {
        return Some("publish: unit record missing".to_string());
    };
    if unit_catalog.get(&unit.definition_id).is_none() {
        return Some("publish: definition missing".to_string());
    }
    let Some(signature) = portrait_signature_for_unit(
        world,
        unit,
        unit_catalog,
        appearance_profiles,
        items,
        visuals,
    ) else {
        return Some("publish: signature failed".to_string());
    };
    if probe.probe_cache_units.contains(&unit_id) {
        cache.remove_unit(unit_id);
    }
    let generation = cache.request_capture(unit_id, signature);
    let committed = cache.commit_capture(unit_id, generation, signature, probe_image.clone());
    probe.probe_cache_units.insert(unit_id);
    probe.probe_unit = Some(unit_id);
    probe.published_generation = generation;
    probe.published_image_id = Some(probe_image.id());
    Some(format!(
        "publish committed={} unit={:?} image={:?} generation={} signature_digest={}",
        committed,
        unit_id,
        probe_image.id(),
        generation,
        signature.digest
    ))
}

pub fn apply_portrait_lifecycle_probe(
    mut commands: Commands,
    mut probe: ResMut<PortraitLifecycleProbe>,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<UnitPortraitCache>,
    demand: Res<UnitPortraitUiDemand>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage_roots: Query<Entity, With<UnitPortraitStageRoot>>,
    probe_meshes: Query<Entity, With<UnitPortraitLifecycleProbeMesh>>,
    mut camera_targets: Query<(&mut Camera, &mut RenderTarget), With<UnitPortraitCamera>>,
    mut camera_transforms: Query<&mut Transform, With<UnitPortraitCamera>>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    items: Res<ItemCatalog>,
    visuals: Res<EquipmentVisualCatalog>,
) {
    if probe.stage == 0 {
        if probe.on_stage_entered || !probe.probe_cache_units.is_empty() || probe.probe_mesh.is_some() {
            cleanup_probe_state(&mut commands, &mut probe, &mut cache, &probe_meshes);
        }
        probe.on_stage_entered = false;
        return;
    }

    if probe.on_stage_entered && probe.stage == 5 {
        despawn_probe_mesh(&mut commands, &mut probe, &probe_meshes);
        let handle = new_portrait_render_target_image(&mut images);
        probe.probe_image = Some(handle);
        probe.pending_log = Some(format!(
            "lifecycle stage 5: fresh probe_image {:?}",
            probe.probe_image.as_ref().map(|h| h.id())
        ));
    }

    if probe.probe_image.is_none() {
        probe.probe_image = Some(new_portrait_render_target_image(&mut images));
        probe.pending_log = Some(format!(
            "lifecycle: allocated probe_image {:?}",
            probe.probe_image.as_ref().map(|h| h.id())
        ));
    }
    let probe_image = probe.probe_image.clone().unwrap();

    for (mut camera, mut target) in &mut camera_targets {
        camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.08, 0.09, 0.11));
        *target = RenderTarget::Image(probe_image.clone().into());
    }
    for mut transform in &mut camera_transforms {
        *transform = generous_probe_camera_transform();
    }

    let mesh_alive = probe
        .probe_mesh
        .is_some_and(|entity| probe_meshes.get(entity).is_ok());

    let wants_mesh = matches!(probe.stage, 1 | 2 | 5 | 6);
    if wants_mesh && !mesh_alive {
        if !spawn_probe_mesh(
            &mut commands,
            &mut probe,
            &stage_roots,
            &mut meshes,
            &mut materials,
        ) {
            probe.pending_log = Some("lifecycle: failed to spawn probe mesh (no stage root)".to_string());
        }
    }

    if probe.on_stage_entered {
        match probe.stage {
            2 => {
                let unit_id = demand.primary_unit.or(probe.probe_unit);
                if let Some(unit_id) = unit_id {
                    if let Some(msg) = publish_probe_to_cache(
                        &mut probe,
                        &mut cache,
                        unit_id,
                        probe_image.clone(),
                        &world,
                        &unit_catalog,
                        &appearance_profiles,
                        &items,
                        &visuals,
                    ) {
                        probe.pending_log = Some(format!("lifecycle stage 2: {}", msg));
                    }
                } else {
                    probe.pending_log =
                        Some("lifecycle stage 2: select a unit before publish".to_string());
                }
            }
            3 => {
                despawn_probe_mesh(&mut commands, &mut probe, &probe_meshes);
                probe.pending_log = Some(
                    "lifecycle stage 3 (A): mesh removed, camera stays ON — observe HUD; press F9"
                        .to_string(),
                );
            }
            4 => {
                probe.pending_log = Some(
                    "lifecycle stage 4 (A): camera OFF — did green survive comparison A?"
                        .to_string(),
                );
            }
            5 => {
                let unit_id = demand.primary_unit.or(probe.probe_unit);
                if let Some(unit_id) = unit_id {
                    if let Some(msg) = publish_probe_to_cache(
                        &mut probe,
                        &mut cache,
                        unit_id,
                        probe_image.clone(),
                        &world,
                        &unit_catalog,
                        &appearance_profiles,
                        &items,
                        &visuals,
                    ) {
                        probe.pending_log = Some(format!("lifecycle stage 5 (B fresh): {}", msg));
                    }
                } else {
                    probe.pending_log =
                        Some("lifecycle stage 5: select a unit before B publish".to_string());
                }
            }
            6 => {
                probe.pending_log = Some(
                    "lifecycle stage 6 (B): camera OFF, mesh still present — safe order"
                        .to_string(),
                );
            }
            7 => {
                despawn_probe_mesh(&mut commands, &mut probe, &probe_meshes);
                probe.pending_log = Some(
                    "lifecycle stage 7 (B): mesh removed after camera off — compare to stage 3–4"
                        .to_string(),
                );
            }
            _ => {}
        }
        log_stage_entered(&mut probe);
        probe.on_stage_entered = false;
    }
}

fn log_stage_entered(probe: &mut PortraitLifecycleProbe) {
    if probe.last_logged_stage == probe.stage {
        return;
    }
    probe.last_logged_stage = probe.stage;
    let image = probe.probe_image.as_ref().map(|h| h.id());
    match probe.stage {
        1 => info!(
            "portrait lifecycle stage 1: probe mesh + camera ON, HUD direct image {:?}",
            image
        ),
        2 => info!("portrait lifecycle stage 2: publish {:?} (mesh+camera ON)", image),
        3 => info!("portrait lifecycle stage 3: comparison A — mesh removed, camera ON"),
        4 => info!("portrait lifecycle stage 4: comparison A — camera OFF"),
        5 => info!("portrait lifecycle stage 5: comparison B — fresh publish {:?}", image),
        6 => info!("portrait lifecycle stage 6: comparison B — camera OFF before mesh removal"),
        7 => info!("portrait lifecycle stage 7: comparison B — mesh removed"),
        _ => {}
    }
}

pub fn record_portrait_lifecycle_trace(
    mut probe: ResMut<PortraitLifecycleProbe>,
    mut trace: ResMut<PortraitLifecycleTrace>,
    capture: Res<UnitPortraitCaptureState>,
    demand: Res<UnitPortraitUiDemand>,
    cache: Res<UnitPortraitCache>,
    diagnostic: Res<super::diagnostics::PortraitDiagnosticTrace>,
    cameras: Query<(&Camera, &RenderTarget), With<UnitPortraitCamera>>,
    actors: Query<Entity, With<UnitPortraitActor>>,
    probe_meshes: Query<Entity, With<UnitPortraitLifecycleProbeMesh>>,
) {
    trace.stage = probe.stage;
    trace.published_generation = probe.published_generation;
    trace.capture_active = capture.active_request.is_some();
    trace.capture_target_id = capture.target_image.as_ref().map(|h| h.id());
    trace.probe_image_id = probe.probe_image.as_ref().map(|h| h.id());
    trace.actor_entity_alive = capture
        .actor_entity
        .is_some_and(|entity| actors.get(entity).is_ok());
    trace.probe_mesh_alive = probe
        .probe_mesh
        .is_some_and(|entity| probe_meshes.get(entity).is_ok());
    trace.ui_image_id = diagnostic.ui_image_id;
    trace.cache_image_id = demand
        .primary_unit
        .and_then(|id| cache.image_for_unit(id).map(|h| h.id()));
    if let Some((camera, target)) = cameras.iter().next() {
        trace.camera_active = camera.is_active;
        trace.camera_target_id = match target {
            RenderTarget::Image(image) => Some(image.handle.id()),
            _ => None,
        };
    }

    if let Some(message) = probe.pending_log.take() {
        info!(
            "portrait lifecycle | {} | stage={} gen={} ui={:?} cache={:?} probe={:?} published={:?} cam_target={:?} cam_active={} capture={} cap_target={:?} actor={} probe_mesh={}",
            message,
            trace.stage,
            trace.published_generation,
            trace.ui_image_id,
            trace.cache_image_id,
            trace.probe_image_id,
            probe.published_image_id,
            trace.camera_target_id,
            trace.camera_active,
            trace.capture_active,
            trace.capture_target_id,
            trace.actor_entity_alive,
            trace.probe_mesh_alive,
        );
    }
}

/// Production `finish_capture` logging while stage 0.
pub fn note_production_finish_capture(
    probe: &mut PortraitLifecycleProbe,
    committed: bool,
    published_image: Option<AssetId<Image>>,
    camera_active_at_despawn: bool,
) {
    if probe.stage != 0 {
        return;
    }
    probe.pending_log = Some(format!(
        "production finish_capture: committed={} published_image={:?} camera_active_at_despawn={}",
        committed,
        published_image,
        camera_active_at_despawn
    ));
}
