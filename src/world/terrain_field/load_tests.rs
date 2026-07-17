//! World-package load tests (ADR-101 TF1).

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::world::{
        ChunkCoord, DEFAULT_TERRAIN_FIELD_MANIFEST_PATH, TERRAIN_FIELD_CATALOG_RON_PATH,
        TERRAIN_FIELD_SAMPLES_PER_TILE, TerrainFieldCatalog, TerrainFieldId, TerrainFieldStore,
        bootstrap_constant_field, decode_manifest, decode_tile, load_terrain_fields_from_manifest,
        try_load_terrain_fields_from_manifest,
    };

    #[test]
    fn production_catalog_ron_loads() {
        let catalog =
            TerrainFieldCatalog::load_from_ron_path(Path::new(TERRAIN_FIELD_CATALOG_RON_PATH))
                .expect("catalog ron");
        assert!(catalog.get(&TerrainFieldId::new("water")).is_some());
        assert_eq!(catalog.sorted_ids().len(), 4);
    }

    #[test]
    fn manifest_round_trip() {
        let text = std::fs::read_to_string(DEFAULT_TERRAIN_FIELD_MANIFEST_PATH).unwrap();
        let manifest = decode_manifest(&text).unwrap();
        assert_eq!(manifest.fields.len(), 4);
        assert_eq!(manifest.config.samples_per_edge, 33);
    }

    #[test]
    fn fixture_water_tile_decodes() {
        let path = "assets/worlds/main/terrain_fields/water/0_0.ron";
        let text = std::fs::read_to_string(path).unwrap();
        let tile = decode_tile(&text).unwrap();
        assert_eq!(tile.chunk, ChunkCoord::new(0, 0));
        assert_eq!(tile.samples.len(), TERRAIN_FIELD_SAMPLES_PER_TILE);
    }

    #[test]
    fn load_manifest_into_store() {
        let catalog =
            TerrainFieldCatalog::load_from_ron_path(Path::new(TERRAIN_FIELD_CATALOG_RON_PATH))
                .unwrap();
        let mut store = TerrainFieldStore::new();
        let config = crate::world::WorldConfig::default();
        let summary = load_terrain_fields_from_manifest(
            &mut store,
            &catalog,
            Path::new(DEFAULT_TERRAIN_FIELD_MANIFEST_PATH),
            &config,
        )
        .expect("load fields");
        assert!(summary.tiles_loaded >= 1);
        assert!(store.has_field_data(&TerrainFieldId::new("water")));
    }

    #[test]
    fn missing_manifest_returns_none() {
        let catalog = TerrainFieldCatalog::default();
        let mut store = TerrainFieldStore::new();
        let config = crate::world::WorldConfig::default();
        assert!(
            try_load_terrain_fields_from_manifest(
                &mut store,
                &catalog,
                Path::new("assets/worlds/missing/terrain_fields/manifest.ron"),
                &config,
            )
            .is_none()
        );
    }

    #[test]
    fn synthetic_constant_tile_round_trip() {
        let mut store = TerrainFieldStore::new();
        bootstrap_constant_field(
            &mut store,
            TerrainFieldId::new("water"),
            ChunkCoord::new(1, 2),
            12_345,
        );
        let tile = store
            .get_tile(&TerrainFieldId::new("water"), ChunkCoord::new(1, 2))
            .unwrap();
        assert_eq!(tile.samples[0], 12_345);
    }

    #[test]
    #[ignore = "manual: writes assets/worlds/main/terrain_fields/water/0_0.ron"]
    fn write_fixture_water_tile() {
        use crate::world::{TerrainFieldTile, TerrainFieldTileFile};

        let tile = crate::world::TerrainFieldTile::new_constant(
            ChunkCoord::new(0, 0),
            25_000,
            "tf1_fixture_v1",
        );
        let file = TerrainFieldTileFile::from_tile(&TerrainFieldId::new("water"), &tile);
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).unwrap();
        std::fs::create_dir_all("assets/worlds/main/terrain_fields/water").unwrap();
        std::fs::write("assets/worlds/main/terrain_fields/water/0_0.ron", text).unwrap();
    }
}
