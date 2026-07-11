//! Skybox cubemap loading and offline face merge utilities.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use bevy::{
    asset::LoadState,
    core_pipeline::Skybox,
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
};

use crate::camera::RtsCamera;

use super::settings::{EnvironmentSettings, SKYBOX_ASSET_ROOT};

/// Cubemap filenames inside a skybox set folder.
pub const CUBEMAP_KTX2_FILE: &str = "cubemap.ktx2";
pub const CUBEMAP_PNG_FILE: &str = "cubemap.png";

/// Loose face filenames in stacked-cubemap order (Bevy / OpenGL cubemap axes).
pub const FACE_FILES_STACK_ORDER: [&str; 6] = [
    "right.png",  // +X
    "left.png",   // −X
    "top.png",    // +Y
    "bottom.png", // −Y
    "front.png",  // +Z
    "back.png",   // −Z
];

/// Resolved asset paths for one skybox set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkyboxCubemapPaths {
    pub set_name: String,
    pub ktx2: String,
    pub png: String,
}

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

/// Tracks whether startup attempted to load a skybox (for dev diagnostics).
#[derive(Resource, Debug, Default)]
pub struct SkyboxLoadStatus {
    pub attempted: bool,
    pub loaded: bool,
}

/// Asset-server paths for cubemap files in `assets/environment/skyboxes/{set_name}/`.
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

/// Directory containing loose face PNGs for one skybox set.
pub fn skybox_set_dir(set_name: &str) -> PathBuf {
    disk_asset_path(&format!("{SKYBOX_ASSET_ROOT}/{set_name}"))
}

/// Output path for merged cubemap PNG.
pub fn merged_cubemap_path(set_name: &str) -> PathBuf {
    skybox_set_dir(set_name).join(CUBEMAP_PNG_FILE)
}

/// Whether all six loose face PNGs exist for `set_name`.
pub fn loose_faces_exist(set_name: &str) -> bool {
    let dir = skybox_set_dir(set_name);
    FACE_FILES_STACK_ORDER
        .iter()
        .all(|name| dir.join(name).is_file())
}

/// Merge loose faces into `assets/environment/skyboxes/{set_name}/cubemap.png`.
pub fn merge_loose_faces(set_name: &str) -> Result<PathBuf, String> {
    let set_dir = skybox_set_dir(set_name);
    if !set_dir.is_dir() {
        return Err(format!(
            "skybox set directory not found: {}",
            set_dir.display()
        ));
    }

    let face_paths: Vec<PathBuf> = FACE_FILES_STACK_ORDER
        .iter()
        .map(|name| set_dir.join(name))
        .collect();

    for path in &face_paths {
        if !path.is_file() {
            return Err(format!(
                "missing face `{}` (expected all of: {})",
                path.display(),
                FACE_FILES_STACK_ORDER.join(", ")
            ));
        }
    }

    let mut face_rgba: Vec<(u32, u32, Vec<u8>)> = Vec::with_capacity(6);
    for path in &face_paths {
        face_rgba.push(decode_png_rgba(path)?);
    }

    let (face_w, face_h) = (face_rgba[0].0, face_rgba[0].1);
    if face_w != face_h {
        return Err(format!(
            "each cubemap face must be square; got {face_w}×{face_h} in {}",
            face_paths[0].display()
        ));
    }
    for (index, (w, h, _)) in face_rgba.iter().enumerate().skip(1) {
        if *w != face_w || *h != face_h {
            return Err(format!(
                "all faces must share dimensions ({face_w}×{face_h}); face {} is {w}×{h}",
                FACE_FILES_STACK_ORDER[index]
            ));
        }
    }

    let out_width = face_w;
    let out_height = face_h * 6;
    let mut stacked = vec![0u8; (out_width * out_height * 4) as usize];

    for (layer, (_w, _h, rgba)) in face_rgba.iter().enumerate() {
        let row_bytes = (face_w * 4) as usize;
        let y_offset = (layer as u32 * face_h) as usize;
        for row in 0..face_h as usize {
            let src_start = row * row_bytes;
            let src_end = src_start + row_bytes;
            let dst_row = y_offset + row;
            let dst_start = dst_row * row_bytes;
            stacked[dst_start..dst_start + row_bytes].copy_from_slice(&rgba[src_start..src_end]);
        }
    }

    let output = merged_cubemap_path(set_name);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {}: {err}", parent.display()))?;
    }

    write_rgba_png(&output, out_width, out_height, &stacked)?;
    Ok(output)
}

fn decode_png_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let file = File::open(path).map_err(|err| format!("open {}: {err}", path.display()))?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .map_err(|err| format!("read {}: {err}", path.display()))?;
    if reader.info().bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "{}: only 8-bit PNG faces are supported",
            path.display()
        ));
    }

    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|err| format!("decode {}: {err}", path.display()))?;

    let width = info.width;
    let height = info.height;
    let pixels = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let rgb = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in rgb.chunks_exact(3) {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            rgba
        }
        other => {
            return Err(format!(
                "{}: unsupported PNG color type {:?}",
                path.display(),
                other
            ));
        }
    };

    Ok((width, height, pixels))
}

fn write_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let file = File::create(path).map_err(|err| format!("create {}: {err}", path.display()))?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|err| format!("write header {}: {err}", path.display()))?;
    writer
        .write_image_data(rgba)
        .map_err(|err| format!("write pixels {}: {err}", path.display()))?;
    Ok(())
}

fn configure_stacked_png_as_cubemap(image: &mut Image) -> Result<(), String> {
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

fn image_load_finished(asset_server: &AssetServer, handle: &Handle<Image>) -> bool {
    matches!(
        asset_server.get_load_state(handle),
        Some(LoadState::Loaded) | Some(LoadState::Failed(_))
    )
}

/// Begin loading the active skybox cubemap once at startup.
pub fn init_skybox_load(
    mut commands: Commands,
    settings: Res<EnvironmentSettings>,
    asset_server: Res<AssetServer>,
) {
    let status = SkyboxLoadStatus {
        attempted: true,
        loaded: false,
    };

    let Some((asset_path, _disk)) = resolve_existing_cubemap(&settings.skybox_set) else {
        warn!(
            "Skybox cubemap not found for set `{}` (expected `assets/{}/{}/cubemap.ktx2` \
             or `cubemap.png`); continuing without skybox",
            settings.skybox_set, SKYBOX_ASSET_ROOT, settings.skybox_set
        );
        commands.insert_resource(status);
        return;
    };

    let image = asset_server.load::<Image>(&asset_path);
    commands.insert_resource(ActiveSkyboxLoad {
        image,
        asset_path,
        configured: false,
        warned_missing: false,
    });
    commands.insert_resource(status);
}

/// Attach [`Skybox`] to the RTS camera when the cubemap is ready.
pub fn attach_skybox_to_primary_camera(
    mut commands: Commands,
    settings: Res<EnvironmentSettings>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut status: ResMut<SkyboxLoadStatus>,
    load: Option<ResMut<ActiveSkyboxLoad>>,
    cameras: Query<Entity, (With<Camera3d>, With<RtsCamera>, Without<SkyboxCamera>)>,
) {
    let Some(mut load) = load else {
        return;
    };

    if load.configured {
        status.loaded = true;
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
            brightness: settings.skybox_brightness,
            rotation: settings.skybox_rotation,
        },
        SkyboxCamera,
    ));
    load.configured = true;
    status.loaded = true;
    #[cfg(feature = "dev")]
    bevy::log::info!("Skybox loaded");
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::super::settings::DEFAULT_SKYBOX_SET;
    use super::*;

    fn write_test_face_png(width: u32, height: u32, fill: u8) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let pixels = vec![fill, fill, fill, 255];
            let row = pixels.repeat(width as usize);
            let image: Vec<u8> = row.repeat(height as usize);
            writer.write_image_data(&image).unwrap();
        }
        buf
    }

    #[test]
    fn cubemap_paths_use_environment_root() {
        let paths = cubemap_paths_for_set(DEFAULT_SKYBOX_SET);
        assert_eq!(paths.ktx2, "environment/skyboxes/default/cubemap.ktx2");
        assert_eq!(paths.png, "environment/skyboxes/default/cubemap.png");
    }

    #[test]
    fn custom_set_paths_are_not_hardcoded_to_default() {
        let paths = cubemap_paths_for_set("night_clear");
        assert_eq!(paths.ktx2, "environment/skyboxes/night_clear/cubemap.ktx2");
    }

    #[test]
    fn stack_order_lists_six_cardinal_faces() {
        assert_eq!(FACE_FILES_STACK_ORDER.len(), 6);
        assert!(FACE_FILES_STACK_ORDER.contains(&"front.png"));
    }

    #[test]
    fn merge_loose_faces_builds_vertical_strip() {
        let set = format!("merge_test_{}", std::process::id());
        let dir = skybox_set_dir(&set);
        std::fs::create_dir_all(&dir).unwrap();

        for (index, name) in FACE_FILES_STACK_ORDER.iter().enumerate() {
            let fill = (index as u8 + 1) * 10;
            std::fs::write(dir.join(name), write_test_face_png(2, 2, fill)).unwrap();
        }

        let output = merge_loose_faces(&set).unwrap();
        assert_eq!(output, merged_cubemap_path(&set));

        let merged = std::fs::read(&output).unwrap();
        let decoder = png::Decoder::new(Cursor::new(merged));
        let reader = decoder.read_info().unwrap();
        assert_eq!(reader.info().width, 2);
        assert_eq!(reader.info().height, 12);

        let _ = std::fs::remove_dir_all(skybox_set_dir(&set));
    }
}
