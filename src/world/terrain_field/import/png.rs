//! PNG mask decoding for terrain fields (ADR-102).
//!
//! Treats mask pixels as linear data. Does not apply sRGB gamma correction.

use std::io::Cursor;
use std::path::Path;

use super::super::source::import_config::TerrainFieldImageChannel;
use super::super::source_error::TerrainFieldSourceError;

/// Decoded source raster in native sample values before remap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFieldImage {
    pub width: u32,
    pub height: u32,
    pub samples: Vec<u32>,
}

impl DecodedFieldImage {
    pub fn sample(&self, x: u32, y: u32) -> u32 {
        self.samples[y as usize * self.width as usize + x as usize]
    }
}

pub fn decode_field_png_from_path(
    path: &Path,
) -> Result<DecodedFieldImage, TerrainFieldSourceError> {
    let bytes = std::fs::read(path).map_err(|err| {
        TerrainFieldSourceError::SourceImageMissing(format!("{}: {err}", path.display()))
    })?;
    decode_field_png_bytes(&bytes)
}

pub fn decode_field_png_bytes(bytes: &[u8]) -> Result<DecodedFieldImage, TerrainFieldSourceError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|err| TerrainFieldSourceError::SourceImageDecodeFailed(err.to_string()))?;

    let width = reader.info().width;
    let height = reader.info().height;
    if width <= 1 || height <= 1 {
        return Err(TerrainFieldSourceError::SourceImageEmpty);
    }

    let bit_depth = reader.info().bit_depth;
    let color_type = reader.info().color_type;
    let row_bytes = reader.output_buffer_size();
    let mut buf = vec![0u8; row_bytes];
    reader
        .next_frame(&mut buf)
        .map_err(|err| TerrainFieldSourceError::SourceImageDecodeFailed(err.to_string()))?;

    let samples = match (color_type, bit_depth) {
        (png::ColorType::Grayscale, png::BitDepth::Eight) => decode_gray8(&buf, width, height),
        (png::ColorType::Grayscale, png::BitDepth::Sixteen) => decode_gray16(&buf, width, height),
        (png::ColorType::Rgb, png::BitDepth::Eight) => {
            decode_rgb8(&buf, width, height, TerrainFieldImageChannel::Luminance)
        }
        (png::ColorType::Rgba, png::BitDepth::Eight) => {
            decode_rgba8(&buf, width, height, TerrainFieldImageChannel::Luminance)
        }
        (other_type, other_depth) => {
            return Err(TerrainFieldSourceError::UnsupportedImageFormat(format!(
                "{other_type:?} @ {other_depth:?}"
            )));
        }
    }?;

    Ok(DecodedFieldImage {
        width,
        height,
        samples,
    })
}

pub fn decode_field_png_with_channel(
    bytes: &[u8],
    channel: TerrainFieldImageChannel,
) -> Result<DecodedFieldImage, TerrainFieldSourceError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|err| TerrainFieldSourceError::SourceImageDecodeFailed(err.to_string()))?;
    let width = reader.info().width;
    let height = reader.info().height;
    if width <= 1 || height <= 1 {
        return Err(TerrainFieldSourceError::SourceImageEmpty);
    }
    let row_bytes = reader.output_buffer_size();
    let mut buf = vec![0u8; row_bytes];
    reader
        .next_frame(&mut buf)
        .map_err(|err| TerrainFieldSourceError::SourceImageDecodeFailed(err.to_string()))?;

    let samples = match reader.info().color_type {
        png::ColorType::Rgb => decode_rgb8(&buf, width, height, channel),
        png::ColorType::Rgba => decode_rgba8(&buf, width, height, channel),
        png::ColorType::Grayscale => decode_field_png_bytes(bytes).map(|d| d.samples),
        other => Err(TerrainFieldSourceError::SourceImageChannelUnavailable(
            format!("{other:?}"),
        )),
    }?;

    Ok(DecodedFieldImage {
        width,
        height,
        samples,
    })
}

fn decode_gray8(buf: &[u8], width: u32, height: u32) -> Result<Vec<u32>, TerrainFieldSourceError> {
    let expected = (width * height) as usize;
    if buf.len() < expected {
        return Err(TerrainFieldSourceError::SourceImageDecodeFailed(
            "truncated grayscale 8-bit buffer".to_string(),
        ));
    }
    Ok(buf[..expected]
        .iter()
        .map(|&v| expand_u8_to_u16(v) as u32)
        .collect())
}

fn decode_gray16(buf: &[u8], width: u32, height: u32) -> Result<Vec<u32>, TerrainFieldSourceError> {
    let expected = (width * height) as usize * 2;
    if buf.len() < expected {
        return Err(TerrainFieldSourceError::SourceImageDecodeFailed(
            "truncated grayscale 16-bit buffer".to_string(),
        ));
    }
    let mut out = Vec::with_capacity((width * height) as usize);
    for chunk in buf[..expected].chunks_exact(2) {
        let value = u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        out.push(value);
    }
    Ok(out)
}

fn decode_rgb8(
    buf: &[u8],
    width: u32,
    height: u32,
    channel: TerrainFieldImageChannel,
) -> Result<Vec<u32>, TerrainFieldSourceError> {
    let expected = (width * height) as usize * 3;
    if buf.len() < expected {
        return Err(TerrainFieldSourceError::SourceImageDecodeFailed(
            "truncated rgb buffer".to_string(),
        ));
    }
    Ok(buf[..expected]
        .chunks_exact(3)
        .map(|px| channel_value_u8(px, channel))
        .collect())
}

fn decode_rgba8(
    buf: &[u8],
    width: u32,
    height: u32,
    channel: TerrainFieldImageChannel,
) -> Result<Vec<u32>, TerrainFieldSourceError> {
    let expected = (width * height) as usize * 4;
    if buf.len() < expected {
        return Err(TerrainFieldSourceError::SourceImageDecodeFailed(
            "truncated rgba buffer".to_string(),
        ));
    }
    Ok(buf[..expected]
        .chunks_exact(4)
        .map(|px| channel_value_u8(px, channel))
        .collect())
}

fn channel_value_u8(px: &[u8], channel: TerrainFieldImageChannel) -> u32 {
    let value = match channel {
        TerrainFieldImageChannel::Red => px[0],
        TerrainFieldImageChannel::Green => px[1],
        TerrainFieldImageChannel::Blue => px[2],
        TerrainFieldImageChannel::Alpha => px.get(3).copied().unwrap_or(255),
        TerrainFieldImageChannel::Luminance => {
            ((px[0] as u16 + px[1] as u16 + px[2] as u16) / 3) as u8
        }
    };
    expand_u8_to_u16(value) as u32
}

/// Deterministic 8-bit → stored sample expansion (ADR-102).
pub fn expand_u8_to_u16(value: u8) -> u16 {
    (value as u16) * 257
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_gray8_png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, width, height);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
        }
        buf
    }

    #[test]
    fn eight_bit_expands_with_times_257() {
        assert_eq!(expand_u8_to_u16(255), 65_535);
        assert_eq!(expand_u8_to_u16(0), 0);
    }

    #[test]
    fn decodes_grayscale_png() {
        let png = encode_gray8_png(2, 2, &[10, 20, 30, 40]);
        let decoded = decode_field_png_bytes(&png).unwrap();
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.sample(1, 0), expand_u8_to_u16(20) as u32);
    }
}
