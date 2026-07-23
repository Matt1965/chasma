//! Dev scene UI actions (ADR-045).

use bevy::prelude::*;

use crate::camera::RtsCameraState;
use crate::world::{
    BuildingCatalog, DoodadCatalog, FootprintCatalog, InteriorProfileCatalog,
    BuildingNavigationBlueprintCatalog, UnitCatalog, WorldData,
};

use super::{
    SceneApplyReport, SceneCaptureContext, SceneDebugFlagsSnapshot, SceneRegistry, apply_scene,
    capture_scene, clear_world_entities,
};

/// Bevy resource wrapping the on-disk scene registry.
#[derive(Resource, Debug)]
pub struct DevSceneRegistry {
    pub registry: SceneRegistry,
}

impl Default for DevSceneRegistry {
    fn default() -> Self {
        Self {
            registry: SceneRegistry::with_default_dir(),
        }
    }
}

pub fn init_dev_scene_registry(mut scenes: ResMut<DevSceneRegistry>) {
    if let Err(err) = scenes.registry.load_from_disk() {
        warn!("dev scene registry load failed: {err}");
    }
}

pub fn save_current_world(
    world: &WorldData,
    registry: &mut SceneRegistry,
    name: &str,
    world_seed: u64,
    debug_flags: Option<SceneDebugFlagsSnapshot>,
    camera: Option<&RtsCameraState>,
) -> Result<String, String> {
    let camera_state = camera.map(|state| super::snapshot::SceneCameraState {
        position: [state.focus.x, state.focus.y, state.focus.z],
        yaw: state.yaw,
        pitch: state.pitch,
    });
    let ctx = SceneCaptureContext::from_dev_state(
        name,
        format!(
            "Dev snapshot saved at {}",
            super::registry::unix_timestamp_secs()
        ),
        world_seed,
        debug_flags,
        camera_state,
    );
    let scene = capture_scene(world, &ctx);
    let scene_id = scene.scene_id.clone();
    registry
        .upsert(scene)
        .map(|_| scene_id)
        .map_err(|err| err.to_string())
}

pub fn load_scene_by_id(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    nav_catalog: Option<&BuildingNavigationBlueprintCatalog>,
    registry: &SceneRegistry,
    scene_id: &str,
) -> Result<SceneApplyReport, String> {
    let scene = registry
        .load_scene(scene_id)
        .map_err(|err| err.to_string())?;
    apply_scene(
        world,
        unit_catalog,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        interior_catalog,
        nav_catalog,
        &scene,
    )
    .map_err(|err| err.to_string())
}

pub fn delete_scene(registry: &mut SceneRegistry, scene_id: &str) -> Result<(), String> {
    registry.delete(scene_id).map_err(|err| err.to_string())
}

pub fn clear_dev_world(world: &mut WorldData) {
    clear_world_entities(world);
}
