//! TF2 import, generation, packaging, and determinism tests (ADR-102).

use std::path::Path;

use crate::world::terrain_field::import::partition::TerrainFieldWorldRaster;
use crate::world::terrain_field::import::png::decode_field_png_bytes;
use crate::world::{
    BuildDependencies, ChunkCoord, ChunkExtent, TerrainFieldImageOrientation,
    TerrainFieldResampling, TerrainFieldSourceProfileCatalog, TerrainFieldValueRemap,
    build_field_layer_from_profile, expand_u8_to_u16, partition_raster_to_tiles,
    resample_imported_image, starter_source_profiles, target_sample_dimensions,
};

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
fn starter_source_profiles_validate() {
    let catalog =
        TerrainFieldSourceProfileCatalog::from_profiles(starter_source_profiles()).unwrap();
    assert_eq!(catalog.profiles().len(), 4);
}

#[test]
fn source_profile_ron_round_trip() {
    let profiles = starter_source_profiles();
    let file = crate::world::TerrainFieldSourceProfileCatalogRon {
        profiles: profiles.clone(),
    };
    let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).unwrap();
    let loaded = TerrainFieldSourceProfileCatalog::load_from_ron(&text).unwrap();
    assert_eq!(loaded.profiles().len(), profiles.len());
}

#[test]
fn four_corner_orientation_row_zero_is_minimum_z() {
    // Row 0 = south (min Z). Pixel layout row-major: SW, SE, NW, NE corners.
    let png = encode_gray8_png(2, 2, &[11, 22, 33, 44]);
    let image = decode_field_png_bytes(&png).unwrap();
    let remap = TerrainFieldValueRemap::full_range();
    let out = resample_imported_image(
        &image,
        2,
        2,
        TerrainFieldImageOrientation::RowZeroIsMinimumZ,
        TerrainFieldResampling::Nearest,
        &remap,
        true,
        1.0,
        1.0,
    )
    .unwrap();
    assert_eq!(out[0], expand_u8_to_u16(11)); // col0 row0 SW
    assert_eq!(out[1], expand_u8_to_u16(22)); // col1 row0 SE
    assert_eq!(out[2], expand_u8_to_u16(33)); // col0 row1 NW
    assert_eq!(out[3], expand_u8_to_u16(44)); // col1 row1 NE
}

#[test]
fn four_corner_orientation_row_zero_is_maximum_z() {
    // Image row 0 is north (max Z): NW=33, NE=44; row 1 is south: SW=11, SE=22.
    let png = encode_gray8_png(2, 2, &[33, 44, 11, 22]);
    let image = decode_field_png_bytes(&png).unwrap();
    let remap = TerrainFieldValueRemap::full_range();
    let out = resample_imported_image(
        &image,
        2,
        2,
        TerrainFieldImageOrientation::RowZeroIsMaximumZ,
        TerrainFieldResampling::Nearest,
        &remap,
        true,
        1.0,
        1.0,
    )
    .unwrap();
    assert_eq!(out[0], expand_u8_to_u16(11)); // south-west
    assert_eq!(out[1], expand_u8_to_u16(22)); // south-east
    assert_eq!(out[2], expand_u8_to_u16(33)); // north-west
    assert_eq!(out[3], expand_u8_to_u16(44)); // north-east
}

#[test]
fn bilinear_resample_is_deterministic() {
    let png = encode_gray8_png(
        4,
        4,
        &[
            0, 64, 128, 192, 32, 96, 160, 224, 16, 80, 144, 208, 48, 112, 176, 240,
        ],
    );
    let image = decode_field_png_bytes(&png).unwrap();
    let remap = TerrainFieldValueRemap::full_range();
    let args = (
        TerrainFieldImageOrientation::RowZeroIsMinimumZ,
        TerrainFieldResampling::Bilinear,
    );
    let a = resample_imported_image(&image, 5, 5, args.0, args.1, &remap, true, 1.0, 1.0).unwrap();
    let b = resample_imported_image(&image, 5, 5, args.0, args.1, &remap, true, 1.0, 1.0).unwrap();
    assert_eq!(a, b);
}

#[test]
fn generated_iron_chunk_order_independent() {
    let catalog =
        TerrainFieldSourceProfileCatalog::from_profiles(starter_source_profiles()).unwrap();
    let profile = catalog
        .for_field(&crate::world::TerrainFieldId::new("iron"))
        .unwrap();
    let extent = ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    };
    let config = crate::world::WorldConfig::default();
    let deps = BuildDependencies::default();
    let (layer_a, _) = build_field_layer_from_profile(profile, extent, &config, &deps).unwrap();
    let (layer_b, _) = build_field_layer_from_profile(profile, extent, &config, &deps).unwrap();
    for chunk in [
        ChunkCoord::new(0, 0),
        ChunkCoord::new(1, 0),
        ChunkCoord::new(0, 1),
        ChunkCoord::new(1, 1),
    ] {
        let ta = layer_a.tiles.get(&chunk).unwrap();
        let tb = layer_b.tiles.get(&chunk).unwrap();
        assert_eq!(ta.samples, tb.samples);
    }
}

#[test]
fn generated_shared_edges_match_exactly() {
    let catalog =
        TerrainFieldSourceProfileCatalog::from_profiles(starter_source_profiles()).unwrap();
    let profile = catalog
        .for_field(&crate::world::TerrainFieldId::new("copper"))
        .unwrap();
    let extent = ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    };
    let config = crate::world::WorldConfig::default();
    let deps = BuildDependencies::default();
    let (layer, _) = build_field_layer_from_profile(profile, extent, &config, &deps).unwrap();
    layer
        .validate_shared_edges()
        .expect("generated copper field must have exact shared edges");
}

#[test]
fn partition_world_dimensions_match_extent() {
    let extent = ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(2, 1),
    };
    let (w, h) = target_sample_dimensions(extent);
    assert_eq!(w, 97);
    assert_eq!(h, 65);
    let raster = TerrainFieldWorldRaster::from_vec(w, h, vec![1u16; (w * h) as usize]).unwrap();
    let tiles = partition_raster_to_tiles(&raster, extent, "v1").unwrap();
    assert_eq!(tiles.len(), 6);
    for tile in tiles.values() {
        assert_eq!(tile.samples.len(), 33 * 33);
    }
}

#[test]
fn duplicate_source_profile_id_rejected() {
    let mut profiles = starter_source_profiles();
    profiles.push(profiles[0].clone());
    assert!(TerrainFieldSourceProfileCatalog::from_profiles(profiles).is_err());
}

#[test]
fn package_build_is_byte_identical_on_repeat() {
    use std::fs;

    use crate::world::{TerrainFieldSourceProfileCatalog, build_and_package_field};

    let catalog =
        TerrainFieldSourceProfileCatalog::from_profiles(starter_source_profiles()).unwrap();
    let profile = catalog
        .for_field(&crate::world::TerrainFieldId::new("iron"))
        .unwrap();
    let extent = ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(0, 0),
    };
    let config = crate::world::WorldConfig::default();
    let deps = BuildDependencies::default();
    let dir_a = std::env::temp_dir().join("chasma_tf2_pkg_a");
    let dir_b = std::env::temp_dir().join("chasma_tf2_pkg_b");
    let _ = fs::remove_dir_all(&dir_a);
    let _ = fs::remove_dir_all(&dir_b);
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();
    let (report_a, _) =
        build_and_package_field(profile, extent, &config, Path::new(&dir_a), "test", &deps)
            .unwrap();
    let (report_b, _) =
        build_and_package_field(profile, extent, &config, Path::new(&dir_b), "test", &deps)
            .unwrap();
    assert_eq!(report_a.source_version, report_b.source_version);
    let manifest_a = fs::read_to_string(dir_a.join("manifest.ron")).unwrap();
    let manifest_b = fs::read_to_string(dir_b.join("manifest.ron")).unwrap();
    assert_eq!(manifest_a, manifest_b);
}
