use std::path::{Path, PathBuf};

use bevy::{
    asset::LoadState,
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
};

use super::settings::{DEFAULT_SKYBOX_SET, SKYBOX_ASSET_ROOT};

/// Cubemap filenames inside a skybox set folder (no hardcoded set name).
pub const CUBEMAP_KTX2_FILE: &str = "cubemap.ktx2";
pub const CUBEMAP_PNG_FILE: &str = "cubemap.png";

/// Resolved asset paths for one skybox set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkyboxCubemapPaths {
    pub set_name: String,
    pub ktx2: String,
    pub png: String,
}

/// Asset-server paths for cubemap files in `assets/skyboxes/{set_name}/`.
pub fn cubemap_paths_for_set(set_name: &str) -> SkyboxCubemapPaths {
    let base = format!("{SKYBOX_ASSET_ROOT}/{set_name}");
    SkyboxCubemapPaths {
        set_name: set_name.to_string(),
        ktx2: format!("{base}/{CUBEMAP_KTX2_FILE}"),
        png: format!("{base}/{CUBEMAP_PNG_FILE}"),
    }
}

/// Disk path used for existence checks before loading (`assets/…` from project root).
pub fn disk_asset_path(asset_path: &str) -> PathBuf {
    Path::new("assets").join(asset_path)
}

/// Pick the first cubemap file that exists on disk for `set_name`.
pub fn resolve_existing_cubemap(set_name: &str) -> Option<(String, PathBuf)> {
    let paths = cubemap_paths_for_set(set_name);
    for asset_path in [&paths.ktx2, &paths.png] {
        let disk = disk_asset_path(asset_path);
        if disk.is_file() {
            return Some((asset_path.clone(), disk));
        }
    }
    None
}

/// Reconfigure a stacked PNG (six square faces, height = 6 × width) as a cubemap view.
pub fn configure_stacked_png_as_cubemap(image: &mut Image) -> Result<(), String> {
    if image.texture_descriptor.array_layer_count() != 1 {
        return Ok(());
    }
    let width = image.width();
    let height = image.height();
    if width == 0 || height % width != 0 {
        return Err(format!(
            "cubemap PNG must be a vertical stack of square faces (height divisible by width); got {width}×{height}"
        ));
    }
    let layers = height / width;
    if layers != 6 {
        return Err(format!(
            "cubemap PNG must contain exactly 6 faces; got {layers} layers"
        ));
    }
    image
        .reinterpret_stacked_2d_as_array(layers)
        .map_err(|err| err.to_string())?;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    Ok(())
}

/// Whether `handle` has finished loading (success or failure).
pub fn image_load_finished(asset_server: &AssetServer, handle: &Handle<Image>) -> bool {
    matches!(
        asset_server.get_load_state(handle),
        Some(LoadState::Loaded) | Some(LoadState::Failed(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubemap_paths_use_skybox_root() {
        let paths = cubemap_paths_for_set(DEFAULT_SKYBOX_SET);
        assert_eq!(paths.ktx2, "skyboxes/default/cubemap.ktx2");
        assert_eq!(paths.png, "skyboxes/default/cubemap.png");
    }

    #[test]
    fn custom_set_paths_are_not_hardcoded_to_default() {
        let paths = cubemap_paths_for_set("night_clear");
        assert_eq!(paths.ktx2, "skyboxes/night_clear/cubemap.ktx2");
        assert_eq!(paths.png, "skyboxes/night_clear/cubemap.png");
    }
}
