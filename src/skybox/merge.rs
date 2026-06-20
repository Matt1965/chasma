//! Offline merge of loose cubemap face PNGs into one stacked `cubemap.png`.
//!
//! Face order matches Bevy's stacked-cubemap convention: +X, −X, +Y, −Y, +Z, −Z.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use super::assets::CUBEMAP_PNG_FILE;
use super::settings::SKYBOX_ASSET_ROOT;

/// Loose face filenames in stacked-cubemap order (Bevy / OpenGL cubemap axes).
pub const FACE_FILES_STACK_ORDER: [&str; 6] = [
    "right.png",  // +X
    "left.png",   // −X
    "top.png",    // +Y
    "bottom.png", // −Y
    "front.png",  // +Z
    "back.png",   // −Z
];

/// Directory containing loose face PNGs for one skybox set.
pub fn skybox_set_dir(set_name: &str) -> PathBuf {
    Path::new("assets")
        .join(SKYBOX_ASSET_ROOT)
        .join(set_name)
}

/// Output path for merged cubemap PNG (`assets/skyboxes/{set}/cubemap.png`).
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

/// Merge loose faces into `assets/skyboxes/{set_name}/cubemap.png`.
pub fn merge_loose_faces(set_name: &str) -> Result<PathBuf, String> {
    let set_dir = skybox_set_dir(set_name);
    if !set_dir.is_dir() {
        return Err(format!("skybox set directory not found: {}", set_dir.display()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
    fn stack_order_lists_six_cardinal_faces() {
        assert_eq!(FACE_FILES_STACK_ORDER.len(), 6);
        assert!(FACE_FILES_STACK_ORDER.contains(&"front.png"));
        assert!(FACE_FILES_STACK_ORDER.contains(&"back.png"));
    }

    #[test]
    fn decode_png_rgba_reads_square_face() {
        let bytes = write_test_face_png(2, 2, 42);
        let dir = std::env::temp_dir().join(format!(
            "chasma_skybox_merge_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("face.png");
        std::fs::write(&path, bytes).unwrap();
        let (w, h, rgba) = decode_png_rgba(&path).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(rgba.len(), 16);
        assert_eq!(rgba[0], 42);
        let _ = std::fs::remove_dir_all(dir);
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
