use std::io::Cursor;
use std::path::Path;

use super::error::BiomeImportError;
use super::mapping::BiomeColorMapping;
use super::mask::{BiomeMask, BiomeMaskBounds};

/// Import a biome mask PNG from disk (ADR-024).
pub fn import_biome_mask_from_png(
    path: impl AsRef<Path>,
    bounds: BiomeMaskBounds,
    mapping: &BiomeColorMapping,
) -> Result<BiomeMask, BiomeImportError> {
    let bytes = std::fs::read(path.as_ref()).map_err(|err| BiomeImportError::Io(err.to_string()))?;
    import_biome_mask_from_png_bytes(&bytes, bounds, mapping)
}

/// Import a biome mask PNG from memory (ADR-024).
pub fn import_biome_mask_from_png_bytes(
    bytes: &[u8],
    bounds: BiomeMaskBounds,
    mapping: &BiomeColorMapping,
) -> Result<BiomeMask, BiomeImportError> {
    let (width, height, rgba, bytes_per_pixel) = decode_png_rgba(bytes)?;
    BiomeMask::from_rgba_rows(width, height, bounds, &rgba, bytes_per_pixel, mapping)
}

fn decode_png_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>, usize), BiomeImportError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|err| BiomeImportError::PngDecode(err.to_string()))?;

    if reader.info().bit_depth != png::BitDepth::Eight {
        return Err(BiomeImportError::UnsupportedBitDepth {
            bit_depth: format!("{:?}", reader.info().bit_depth),
        });
    }

    let width = reader.info().width;
    let height = reader.info().height;
    if width == 0 || height == 0 {
        return Err(BiomeImportError::EmptyImage);
    }

    let bytes_per_pixel = match reader.info().color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => {
            return Err(BiomeImportError::UnsupportedColorType {
                color_type: format!("{other:?}"),
            });
        }
    };

    let row_bytes = reader.output_buffer_size();
    let mut buf = vec![0u8; row_bytes];
    reader
        .next_frame(&mut buf)
        .map_err(|err| BiomeImportError::PngDecode(err.to_string()))?;

    Ok((width, height, buf, bytes_per_pixel))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::world::biome::id::BiomeId;

    fn encode_test_png(pixels: &[(u8, u8, u8)], width: u32, height: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, width, height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let data: Vec<u8> = pixels
                .iter()
                .flat_map(|(r, g, b)| [*r, *g, *b])
                .collect();
            writer.write_image_data(&data).unwrap();
        }
        buf
    }

    #[test]
    fn imports_rgb_png_and_classifies_pixels() {
        let png = encode_test_png(
            &[
                (255, 0, 0),
                (0, 255, 0),
                (0, 0, 255),
                (255, 255, 0),
            ],
            2,
            2,
        );
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 4.0, 4.0);
        let mask = import_biome_mask_from_png_bytes(&png, bounds, &BiomeColorMapping::starter()).unwrap();

        assert_eq!(mask.width(), 2);
        assert_eq!(mask.height(), 2);
        assert_eq!(mask.pixel_biome(0, 0), BiomeId::Desert);
        assert_eq!(mask.pixel_biome(1, 0), BiomeId::Forest);
        assert_eq!(mask.pixel_biome(0, 1), BiomeId::Marsh);
        assert_eq!(mask.pixel_biome(1, 1), BiomeId::Plains);
    }

    #[test]
    fn invalid_color_becomes_unassigned_at_import() {
        let png = encode_test_png(&[(128, 64, 32); 4], 2, 2);
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 4.0, 4.0);
        let mask = import_biome_mask_from_png_bytes(&png, bounds, &BiomeColorMapping::starter()).unwrap();
        assert_eq!(mask.pixel_biome(0, 0), BiomeId::Unassigned);
    }

    #[test]
    fn rejects_unsupported_png_color_type() {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, 1, 1);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[128]).unwrap();
        }
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 1.0, 1.0);
        let err = import_biome_mask_from_png_bytes(&buf, bounds, &BiomeColorMapping::starter()).unwrap_err();
        assert!(matches!(err, BiomeImportError::UnsupportedColorType { .. }));
    }

    #[test]
    fn decode_png_via_cursor_matches_bytes_import() {
        let png = encode_test_png(&[(255, 0, 0)], 1, 1);
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 1.0, 1.0);
        let from_bytes =
            import_biome_mask_from_png_bytes(&png, bounds, &BiomeColorMapping::starter()).unwrap();
        let decoder = png::Decoder::new(Cursor::new(&png));
        assert!(decoder.read_info().is_ok());
        assert_eq!(from_bytes.pixel_biome(0, 0), BiomeId::Desert);
    }
}
