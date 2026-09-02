//! Incremental per-field package build + load regression (ADR-102).

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use bevy::prelude::Vec3;

    use crate::world::{
        BuildDependencies, ChunkCoord, ChunkExtent, LocalPosition, TERRAIN_FIELD_MANIFEST_VERSION,
        TERRAIN_FIELD_SAMPLE_SPACING_METERS, TERRAIN_FIELD_SAMPLES_PER_EDGE, TerrainFieldCatalog,
        TerrainFieldId, TerrainFieldManifest, TerrainFieldManifestConfig,
        TerrainFieldManifestEntry, TerrainFieldSourceProfileCatalog, TerrainFieldStore,
        TerrainFieldTile, TerrainFieldTileFile, WorldConfig, WorldPosition,
        build_and_package_field, decode_manifest, load_terrain_fields_from_manifest,
        package_manifest_source_version, sample_terrain_field_at, starter_source_profiles,
    };

    fn tiny_extent() -> ChunkExtent {
        ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(0, 0),
        }
    }

    fn write_manifest(
        dir: &Path,
        entries: Vec<TerrainFieldManifestEntry>,
        package_version: Option<String>,
    ) {
        let config = WorldConfig::default();
        let source_version =
            package_version.unwrap_or_else(|| package_manifest_source_version(&entries));
        let manifest = TerrainFieldManifest {
            version: TERRAIN_FIELD_MANIFEST_VERSION,
            world_id: "test".to_string(),
            source_version,
            config: TerrainFieldManifestConfig {
                chunk_size_meters: config.chunk_size_meters,
                sample_spacing_meters: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
                samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
            },
            fields: entries,
        };
        let text =
            ron::ser::to_string_pretty(&manifest, ron::ser::PrettyConfig::default()).unwrap();
        fs::write(dir.join("manifest.ron"), text).unwrap();
    }

    fn write_constant_tile(
        dir: &Path,
        field_id: &str,
        chunk: ChunkCoord,
        value: u16,
        version: &str,
    ) {
        fs::create_dir_all(dir).unwrap();
        let tile = TerrainFieldTile::new_constant(chunk, value, version);
        let file = TerrainFieldTileFile::from_tile(&TerrainFieldId::new(field_id), &tile);
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).unwrap();
        fs::write(dir.join(format!("{}_{}.ron", chunk.x, chunk.z)), text).unwrap();
    }

    #[test]
    fn multi_field_package_with_distinct_versions_loads() {
        let dir = std::env::temp_dir().join("chasma_tf_pkg_multi_load");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        write_constant_tile(
            &dir.join("water"),
            "water",
            ChunkCoord::new(0, 0),
            10_000,
            "tf2_water_v1",
        );
        write_constant_tile(
            &dir.join("copper"),
            "copper",
            ChunkCoord::new(0, 0),
            20_000,
            "tf2_copper_v1",
        );
        write_manifest(
            &dir,
            vec![
                TerrainFieldManifestEntry {
                    field_id: "copper".to_string(),
                    tile_dir: "copper".to_string(),
                    source_version: Some("tf2_copper_v1".to_string()),
                },
                TerrainFieldManifestEntry {
                    field_id: "water".to_string(),
                    tile_dir: "water".to_string(),
                    source_version: Some("tf2_water_v1".to_string()),
                },
            ],
            None,
        );

        let catalog = TerrainFieldCatalog::default();
        let mut store = TerrainFieldStore::new();
        let summary = load_terrain_fields_from_manifest(
            &mut store,
            &catalog,
            &dir.join("manifest.ron"),
            &WorldConfig::default(),
        )
        .expect("distinct per-field versions should load");
        assert_eq!(summary.tiles_loaded, 2);
        assert_eq!(
            store
                .get_tile(&TerrainFieldId::new("water"), ChunkCoord::new(0, 0))
                .unwrap()
                .samples[0],
            10_000
        );
        assert_eq!(
            store
                .get_tile(&TerrainFieldId::new("copper"), ChunkCoord::new(0, 0))
                .unwrap()
                .samples[0],
            20_000
        );
    }

    #[test]
    fn rebuild_one_field_preserves_other_field_versions() {
        let dir = std::env::temp_dir().join("chasma_tf_pkg_incremental");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let catalog =
            TerrainFieldSourceProfileCatalog::from_profiles(starter_source_profiles()).unwrap();
        let iron_profile = catalog
            .for_field(&TerrainFieldId::new("iron"))
            .expect("iron profile");
        let copper_profile = catalog
            .for_field(&TerrainFieldId::new("copper"))
            .expect("copper profile");
        let extent = tiny_extent();
        let config = WorldConfig::default();
        let deps = BuildDependencies::default();

        build_and_package_field(copper_profile, extent, &config, &dir, "test", &deps)
            .expect("initial copper build");
        let manifest_after_copper =
            decode_manifest(&fs::read_to_string(dir.join("manifest.ron")).unwrap()).unwrap();
        let copper_field_version = manifest_after_copper
            .fields
            .iter()
            .find(|e| e.field_id == "copper")
            .and_then(|e| e.source_version.clone())
            .expect("copper provenance");

        let (iron_report, _) =
            build_and_package_field(iron_profile, extent, &config, &dir, "test", &deps)
                .expect("incremental iron build");

        let manifest =
            decode_manifest(&fs::read_to_string(dir.join("manifest.ron")).unwrap()).unwrap();
        let copper_entry = manifest
            .fields
            .iter()
            .find(|e| e.field_id == "copper")
            .expect("copper entry");
        let iron_entry = manifest
            .fields
            .iter()
            .find(|e| e.field_id == "iron")
            .expect("iron entry");
        assert_eq!(
            copper_entry.source_version.as_deref(),
            Some(copper_field_version.as_str())
        );
        assert_eq!(
            iron_entry.source_version.as_deref(),
            Some(iron_report.source_version.as_str())
        );
        assert_ne!(
            copper_entry.source_version.as_deref(),
            iron_entry.source_version.as_deref()
        );

        let field_catalog = TerrainFieldCatalog::default();
        let mut store = TerrainFieldStore::new();
        load_terrain_fields_from_manifest(
            &mut store,
            &field_catalog,
            &dir.join("manifest.ron"),
            &config,
        )
        .expect("incremental package loads");
        assert!(store.has_field_data(&TerrainFieldId::new("iron")));
        assert!(store.has_field_data(&TerrainFieldId::new("copper")));
    }

    #[test]
    fn stale_tile_version_still_rejected() {
        let dir = std::env::temp_dir().join("chasma_tf_pkg_stale");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        write_constant_tile(
            &dir.join("water"),
            "water",
            ChunkCoord::new(0, 0),
            5_000,
            "tf2_actual",
        );
        write_manifest(
            &dir,
            vec![TerrainFieldManifestEntry {
                field_id: "water".to_string(),
                tile_dir: "water".to_string(),
                source_version: Some("tf2_expected".to_string()),
            }],
            None,
        );

        let catalog = TerrainFieldCatalog::default();
        let mut store = TerrainFieldStore::new();
        let err = load_terrain_fields_from_manifest(
            &mut store,
            &catalog,
            &dir.join("manifest.ron"),
            &WorldConfig::default(),
        )
        .expect_err("manifest/tile mismatch must fail");
        assert!(err.to_string().contains("source version mismatch"));
    }

    #[test]
    fn build_package_reload_sample_path_uses_generated_tiles() {
        let dir = std::env::temp_dir().join("chasma_tf_pkg_reload_sample");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let catalog =
            TerrainFieldSourceProfileCatalog::from_profiles(starter_source_profiles()).unwrap();
        let profile = catalog
            .for_field(&TerrainFieldId::new("iron"))
            .expect("iron");
        let extent = tiny_extent();
        let config = WorldConfig::default();
        let (report, _) = build_and_package_field(
            profile,
            extent,
            &config,
            &dir,
            "test",
            &BuildDependencies::default(),
        )
        .expect("build iron");

        let field_catalog = TerrainFieldCatalog::default();
        let mut store = TerrainFieldStore::new();
        load_terrain_fields_from_manifest(
            &mut store,
            &field_catalog,
            &dir.join("manifest.ron"),
            &config,
        )
        .expect("load after build");

        let layout = config.chunk_layout();
        let mut world = crate::world::WorldData::new(layout);
        world.set_authored_extent(extent);
        *world.terrain_fields_mut() = store;

        let sample = sample_terrain_field_at(
            &world,
            &field_catalog,
            &TerrainFieldId::new("iron"),
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
            ),
        );
        assert!(sample.availability.is_available());
        assert_ne!(
            sample.value, 20_000,
            "must not be dev synthetic water constant"
        );
        assert_eq!(
            world
                .terrain_fields()
                .get_layer(&TerrainFieldId::new("iron"))
                .expect("layer")
                .source_version,
            report.source_version
        );
    }

    #[test]
    fn individual_field_rebuild_after_full_build_still_loads() {
        let dir = std::env::temp_dir().join("chasma_tf_pkg_water_after_full");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let source_catalog =
            TerrainFieldSourceProfileCatalog::from_profiles(starter_source_profiles()).unwrap();
        let extent = tiny_extent();
        let config = WorldConfig::default();
        let deps = BuildDependencies::default();

        for field in ["copper", "iron"] {
            let profile = source_catalog
                .for_field(&TerrainFieldId::new(field))
                .expect("profile");
            build_and_package_field(profile, extent, &config, &dir, "test", &deps)
                .expect("seed field");
        }

        let copper_before = fs::read_to_string(dir.join("copper/0_0.ron")).unwrap();
        let iron_profile = source_catalog
            .for_field(&TerrainFieldId::new("iron"))
            .expect("iron");
        build_and_package_field(iron_profile, extent, &config, &dir, "test", &deps)
            .expect("rebuild iron only");

        assert_eq!(
            fs::read_to_string(dir.join("copper/0_0.ron")).unwrap(),
            copper_before,
            "copper tiles untouched by iron-only rebuild"
        );

        let field_catalog = TerrainFieldCatalog::default();
        let mut store = TerrainFieldStore::new();
        load_terrain_fields_from_manifest(
            &mut store,
            &field_catalog,
            &dir.join("manifest.ron"),
            &config,
        )
        .expect("mixed-version package loads after single-field rebuild");
    }

    #[test]
    fn package_manifest_source_version_is_deterministic() {
        let entries = vec![
            TerrainFieldManifestEntry {
                field_id: "a".to_string(),
                tile_dir: "a".to_string(),
                source_version: Some("tf2_one".to_string()),
            },
            TerrainFieldManifestEntry {
                field_id: "b".to_string(),
                tile_dir: "b".to_string(),
                source_version: Some("tf2_two".to_string()),
            },
        ];
        let a = package_manifest_source_version(&entries);
        let b = package_manifest_source_version(&entries);
        assert_eq!(a, b);
        assert!(a.starts_with("tfp_"));
    }

    #[test]
    fn production_manifest_per_field_versions_match_tiles() {
        let text =
            std::fs::read_to_string(crate::world::DEFAULT_TERRAIN_FIELD_MANIFEST_PATH).unwrap();
        let manifest = decode_manifest(&text).unwrap();
        assert_eq!(manifest.fields.len(), 4);
        for entry in &manifest.fields {
            assert!(
                entry.source_version.is_some(),
                "field {} must declare per-field source_version",
                entry.field_id
            );
        }
        assert_eq!(
            manifest.source_version,
            package_manifest_source_version(&manifest.fields)
        );

        let catalog = TerrainFieldCatalog::default();
        let mut store = TerrainFieldStore::new();
        load_terrain_fields_from_manifest(
            &mut store,
            &catalog,
            std::path::Path::new(crate::world::DEFAULT_TERRAIN_FIELD_MANIFEST_PATH),
            &WorldConfig::default(),
        )
        .expect("production manifest loads with per-field versions");
    }

    #[test]
    fn legacy_manifest_without_per_field_versions_still_loads() {
        let dir = std::env::temp_dir().join("chasma_tf_pkg_legacy");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let shared = "tf2_legacy_shared";
        write_constant_tile(
            &dir.join("water"),
            "water",
            ChunkCoord::new(0, 0),
            9_000,
            shared,
        );
        let config = WorldConfig::default();
        let legacy = TerrainFieldManifest {
            version: TERRAIN_FIELD_MANIFEST_VERSION,
            world_id: "test".to_string(),
            source_version: shared.to_string(),
            config: TerrainFieldManifestConfig {
                chunk_size_meters: config.chunk_size_meters,
                sample_spacing_meters: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
                samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
            },
            fields: vec![TerrainFieldManifestEntry {
                field_id: "water".to_string(),
                tile_dir: "water".to_string(),
                source_version: None,
            }],
        };
        let legacy_text =
            ron::ser::to_string_pretty(&legacy, ron::ser::PrettyConfig::default()).unwrap();
        fs::write(dir.join("manifest.ron"), legacy_text).unwrap();

        let catalog = TerrainFieldCatalog::default();
        let mut store = TerrainFieldStore::new();
        load_terrain_fields_from_manifest(
            &mut store,
            &catalog,
            &dir.join("manifest.ron"),
            &WorldConfig::default(),
        )
        .expect("legacy shared version manifest loads");
    }
}
