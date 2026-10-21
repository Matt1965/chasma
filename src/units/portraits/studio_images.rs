//! Shared portrait texture handles (UI binding + camera render target).

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::studio::PORTRAIT_TEXTURE_SIZE;

/// Stable handles for the portrait HUD slot and capture camera.
#[derive(Resource, Debug, Clone)]
pub struct UnitPortraitStudioImages {
    /// Camera render target; bound to the HUD during live diagnostic steps 2-6.
    pub live_target: Handle<Image>,
    /// CPU checkerboard for diagnostic step 1 (never written by the camera).
    pub checkerboard: Handle<Image>,
}

impl UnitPortraitStudioImages {
    pub fn install(images: &mut Assets<Image>) -> Self {
        let live_target = images.add(new_portrait_render_target());
        let checkerboard = images.add(make_checkerboard_image(PORTRAIT_TEXTURE_SIZE));
        Self {
            live_target,
            checkerboard,
        }
    }
}

pub fn new_portrait_render_target() -> Image {
    Image::new_target_texture(
        PORTRAIT_TEXTURE_SIZE,
        PORTRAIT_TEXTURE_SIZE,
        TextureFormat::Bgra8UnormSrgb,
        None,
    )
}

/// Magenta/cyan 16px tiles — obvious in the HUD if step 1 passes.
pub fn make_checkerboard_image(size: u32) -> Image {
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let checker = ((x / 16) + (y / 16)) % 2 == 0;
            let (r, g, b) = if checker {
                (240, 50, 210)
            } else {
                (50, 220, 240)
            };
            let i = ((y * size + x) * 4) as usize;
            data[i] = r;
            data[i + 1] = g;
            data[i + 2] = b;
            data[i + 3] = 255;
        }
    }
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::portraits::studio::PORTRAIT_TEXTURE_SIZE;

    #[test]
    fn checkerboard_has_expected_dimensions() {
        let image = make_checkerboard_image(PORTRAIT_TEXTURE_SIZE);
        assert_eq!(image.width(), PORTRAIT_TEXTURE_SIZE);
        assert_eq!(image.height(), PORTRAIT_TEXTURE_SIZE);
    }
}
