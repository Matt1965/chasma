use bevy::{
    asset::LoadState,
    core_pipeline::Skybox,
    prelude::*,
    render::render_resource::TextureViewDimension,
};

use crate::camera::RtsCamera;

use super::assets::{
    configure_stacked_png_as_cubemap, image_load_finished, resolve_existing_cubemap,
};
use super::settings::SkyboxSettings;

/// Marker: the primary camera already has a skybox attached (prevents duplicates).
#[derive(Component, Debug)]
pub struct SkyboxCamera;

/// Pending cubemap load state for the active skybox set.
#[derive(Resource, Debug)]
pub struct ActiveSkyboxLoad {
    pub image: Handle<Image>,
    pub asset_path: String,
    pub configured: bool,
    pub warned_missing: bool,
}

/// Begin loading the active skybox cubemap once at startup.
pub fn init_active_skybox_load(
    mut commands: Commands,
    settings: Res<SkyboxSettings>,
    asset_server: Res<AssetServer>,
) {
    let Some((asset_path, _disk)) = resolve_existing_cubemap(&settings.active_set) else {
        warn!(
            "Skybox cubemap not found for set `{}` (expected `assets/skyboxes/{}/cubemap.ktx2` \
             or `cubemap.png`); continuing without skybox",
            settings.active_set, settings.active_set
        );
        return;
    };

    let image = asset_server.load::<Image>(&asset_path);
    info!("Loading skybox from `{asset_path}`");
    commands.insert_resource(ActiveSkyboxLoad {
        image,
        asset_path,
        configured: false,
        warned_missing: false,
    });
}

/// Attach [`Skybox`] to the RTS camera when the cubemap is ready.
pub fn attach_skybox_to_primary_camera(
    mut commands: Commands,
    settings: Res<SkyboxSettings>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    load: Option<ResMut<ActiveSkyboxLoad>>,
    cameras: Query<Entity, (With<Camera3d>, With<RtsCamera>, Without<SkyboxCamera>)>,
) {
    let Some(mut load) = load else {
        return;
    };

    if load.configured {
        return;
    }

    if !image_load_finished(&asset_server, &load.image) {
        return;
    }

    if matches!(
        asset_server.get_load_state(&load.image),
        Some(LoadState::Failed(_))
    ) {
        if !load.warned_missing {
            warn!(
                "Skybox failed to load `{}`; continuing without skybox",
                load.asset_path
            );
            load.warned_missing = true;
        }
        return;
    }

    let Some(image) = images.get_mut(&load.image) else {
        return;
    };

    let dimension = image
        .texture_view_descriptor
        .as_ref()
        .and_then(|desc| desc.dimension);

    if dimension != Some(TextureViewDimension::Cube) {
        if load.asset_path.ends_with(".png") {
            if let Err(err) = configure_stacked_png_as_cubemap(image) {
                if !load.warned_missing {
                    warn!("Skybox PNG `{path}` invalid: {err}", path = load.asset_path);
                    load.warned_missing = true;
                }
                return;
            }
        } else if !load.warned_missing {
            warn!(
                "Skybox `{path}` is not a cubemap texture; continuing without skybox",
                path = load.asset_path
            );
            load.warned_missing = true;
            return;
        }
    }

    let Ok(camera) = cameras.single() else {
        return;
    };

    commands.entity(camera).insert((
        Skybox {
            image: load.image.clone(),
            brightness: settings.brightness,
            rotation: settings.rotation,
        },
        SkyboxCamera,
    ));
    load.configured = true;
    info!("Skybox attached to primary camera (`{}`)", load.asset_path);
}
