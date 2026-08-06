//! One-shot primary window title/icon identity (client presentation only).
//!
//! On Windows, title-bar and taskbar icons are separate (`ICON_SMALL` vs
//! `ICON_BIG`). `set_window_icon` alone updates the title bar; the taskbar
//! requires [`WindowExtWindows::set_taskbar_icon`].
//!
//! Icon install must run on the **main thread**: `WINIT_WINDOWS` is a
//! thread-local populated only there. A normal multithreaded `Update` system
//! can miss the native window forever.

use std::io::Cursor;

use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;
use winit::window::Icon;

#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;

/// Bevy asset-relative path for the window icon (under `assets/`).
pub const WINDOW_ICON_ASSET_PATH: &str = "images/chasma_icon.png";

/// Compile-time icon bytes (static application asset; not a runtime Bevy UI image).
const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../assets/images/chasma_icon.png");

/// Windows title-bar / `ICON_SMALL` size (multiple of the 16px base).
const WINDOW_ICON_SIZE: u32 = 32;
/// Windows taskbar / `ICON_BIG` ceiling recommended by winit.
const TASKBAR_ICON_SIZE: u32 = 256;

/// One-shot install progress for the OS window icons.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowIconInstallState {
    #[default]
    Pending,
    Installed,
    Failed,
}

/// Apply the Chasma window (+ taskbar) icon once the primary winit window exists.
///
/// Forced onto the main thread via [`NonSendMarker`] so `WINIT_WINDOWS` is the
/// same thread-local the winit plugin populated.
pub fn set_window_icon_once(
    _main_thread: NonSendMarker,
    mut state: ResMut<WindowIconInstallState>,
    primary: Query<Entity, With<PrimaryWindow>>,
) {
    if !matches!(*state, WindowIconInstallState::Pending) {
        return;
    }

    let Ok(entity) = primary.single() else {
        return;
    };

    let result = WINIT_WINDOWS.with_borrow(|winit_windows| {
        let Some(window) = winit_windows.get_window(entity) else {
            return IconApply::WindowMissing;
        };
        match build_window_icons(WINDOW_ICON_PNG) {
            Ok((window_icon, taskbar_icon)) => {
                window.set_window_icon(Some(window_icon));
                #[cfg(target_os = "windows")]
                {
                    window.set_taskbar_icon(Some(taskbar_icon));
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = taskbar_icon;
                }
                IconApply::Applied
            }
            Err(err) => IconApply::Failed(err),
        }
    });

    match result {
        IconApply::WindowMissing => {
            // Native window not ready yet; stay Pending and try again.
        }
        IconApply::Applied => {
            *state = WindowIconInstallState::Installed;
            info!(
                "Installed window icons from `{WINDOW_ICON_ASSET_PATH}` (title {WINDOW_ICON_SIZE}px, taskbar {TASKBAR_ICON_SIZE}px)"
            );
        }
        IconApply::Failed(reason) => {
            *state = WindowIconInstallState::Failed;
            warn!(
                "Failed to set window icon from `{WINDOW_ICON_ASSET_PATH}` ({reason}); using OS default"
            );
        }
    }
}

#[derive(Debug)]
enum IconApply {
    WindowMissing,
    Applied,
    Failed(String),
}

fn build_window_icons(bytes: &[u8]) -> Result<(Icon, Icon), String> {
    let (rgba, width, height) = decode_window_icon_rgba(bytes)?;
    let window_rgba = downsample_rgba(&rgba, width, height, WINDOW_ICON_SIZE, WINDOW_ICON_SIZE);
    let taskbar_rgba = downsample_rgba(&rgba, width, height, TASKBAR_ICON_SIZE, TASKBAR_ICON_SIZE);

    let expected_window = (WINDOW_ICON_SIZE as usize) * (WINDOW_ICON_SIZE as usize) * 4;
    let expected_taskbar = (TASKBAR_ICON_SIZE as usize) * (TASKBAR_ICON_SIZE as usize) * 4;
    if window_rgba.len() != expected_window {
        return Err(format!(
            "window icon buffer length {} != {expected_window}",
            window_rgba.len()
        ));
    }
    if taskbar_rgba.len() != expected_taskbar {
        return Err(format!(
            "taskbar icon buffer length {} != {expected_taskbar}",
            taskbar_rgba.len()
        ));
    }

    let window_icon = Icon::from_rgba(window_rgba, WINDOW_ICON_SIZE, WINDOW_ICON_SIZE)
        .map_err(|err| format!("window icon: {err}"))?;
    let taskbar_icon = Icon::from_rgba(taskbar_rgba, TASKBAR_ICON_SIZE, TASKBAR_ICON_SIZE)
        .map_err(|err| format!("taskbar icon: {err}"))?;
    Ok((window_icon, taskbar_icon))
}

/// Decode PNG bytes to RGBA8 without modifying the source asset file.
fn decode_window_icon_rgba(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|err| format!("png decode: {err}"))?;

    if reader.info().bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "unsupported bit depth {:?}",
            reader.info().bit_depth
        ));
    }

    let width = reader.info().width;
    let height = reader.info().height;
    if width == 0 || height == 0 {
        return Err("icon has zero dimensions".into());
    }

    let color_type = reader.info().color_type;
    let mut frame = vec![0u8; reader.output_buffer_size()];
    reader
        .next_frame(&mut frame)
        .map_err(|err| format!("png frame: {err}"))?;

    let rgba = match color_type {
        png::ColorType::Rgba => frame,
        png::ColorType::Rgb => {
            let pixel_count = (width as usize).saturating_mul(height as usize);
            if frame.len() != pixel_count.saturating_mul(3) {
                return Err(format!(
                    "rgb buffer length {} does not match {}x{}",
                    frame.len(),
                    width,
                    height
                ));
            }
            let mut out = Vec::with_capacity(pixel_count.saturating_mul(4));
            for chunk in frame.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        other => return Err(format!("unsupported color type {other:?}")),
    };

    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if rgba.len() != expected {
        return Err(format!(
            "rgba length {} does not match {}x{} (expected {expected})",
            rgba.len(),
            width,
            height
        ));
    }

    Ok((rgba, width, height))
}

/// Box-filter downscale for window API sizes. Source PNG on disk is unchanged.
fn downsample_rgba(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let src_w = src_w as usize;
    let src_h = src_h as usize;
    let dst_w = dst_w as usize;
    let dst_h = dst_h as usize;
    let mut out = vec![0u8; dst_w.saturating_mul(dst_h).saturating_mul(4)];

    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return out;
    }

    for dy in 0..dst_h {
        let y0 = dy * src_h / dst_h;
        let y1 = ((dy + 1) * src_h / dst_h).max(y0 + 1).min(src_h);
        for dx in 0..dst_w {
            let x0 = dx * src_w / dst_w;
            let x1 = ((dx + 1) * src_w / dst_w).max(x0 + 1).min(src_w);
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * src_w + x) * 4;
                    sum[0] += u32::from(src[i]);
                    sum[1] += u32::from(src[i + 1]);
                    sum[2] += u32::from(src[i + 2]);
                    sum[3] += u32::from(src[i + 3]);
                    count += 1;
                }
            }
            let o = (dy * dst_w + dx) * 4;
            if count == 0 {
                continue;
            }
            out[o] = (sum[0] / count) as u8;
            out[o + 1] = (sum[1] / count) as u8;
            out[o + 2] = (sum[2] / count) as u8;
            out[o + 3] = (sum[3] / count) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_icon_decodes_to_validated_rgba() {
        let (rgba, width, height) = decode_window_icon_rgba(WINDOW_ICON_PNG).expect("decode");
        assert_eq!((width, height), (1254, 1254));
        assert_eq!(rgba.len(), (width as usize) * (height as usize) * 4);
        // Source PNG is RGB; decoded path synthesizes opaque alpha.
        assert!(rgba.chunks_exact(4).all(|px| px[3] == 255));
    }

    #[test]
    fn window_icons_build_at_expected_sizes() {
        let (window_icon, taskbar_icon) = build_window_icons(WINDOW_ICON_PNG).expect("icons");
        drop(window_icon);
        drop(taskbar_icon);
    }

    #[test]
    fn install_state_defaults_pending() {
        assert_eq!(
            WindowIconInstallState::default(),
            WindowIconInstallState::Pending
        );
    }
}
