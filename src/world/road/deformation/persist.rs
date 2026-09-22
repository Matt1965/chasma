use std::fs;
use std::path::{Path, PathBuf};

use super::bake::fingerprint_road_network;
use super::store::{
    ROAD_DEFORMATION_BAKE_VERSION, RoadDeformationBakeDocument, RoadDeformationStore,
};
use crate::world::{ChunkId, RoadNetwork};

const BAKED_DIR: &str = "roads/baked";
const BAKE_MANIFEST: &str = "manifest.ron";

pub fn road_deformation_bake_dir(world_package_dir: impl AsRef<Path>) -> PathBuf {
    world_package_dir.as_ref().join(BAKED_DIR)
}

pub fn road_deformation_manifest_path(world_package_dir: impl AsRef<Path>) -> PathBuf {
    road_deformation_bake_dir(world_package_dir).join(BAKE_MANIFEST)
}

pub fn save_road_deformation_bake(
    world_package_dir: impl AsRef<Path>,
    network: &RoadNetwork,
    store: &RoadDeformationStore,
) -> Result<(), String> {
    let dir = road_deformation_bake_dir(&world_package_dir);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let document = RoadDeformationBakeDocument {
        bake_version: store.bake_version,
        network_fingerprint: fingerprint_road_network(network),
        tiles: store
            .tiles
            .iter()
            .map(|(chunk_id, tile)| (chunk_id.coord(), tile.clone()))
            .collect(),
    };
    let serialized = ron::ser::to_string_pretty(
        &document,
        ron::ser::PrettyConfig::new().new_line("\n".to_string()),
    )
    .map_err(|error| error.to_string())?;
    fs::write(road_deformation_manifest_path(&world_package_dir), serialized)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn load_road_deformation_bake(
    world_package_dir: impl AsRef<Path>,
    network: &RoadNetwork,
) -> Option<RoadDeformationStore> {
    let path = road_deformation_manifest_path(world_package_dir);
    if !path.exists() {
        return None;
    }
    let contents = fs::read_to_string(&path).ok()?;
    let document: RoadDeformationBakeDocument = ron::from_str(&contents).ok()?;
    if document.bake_version != ROAD_DEFORMATION_BAKE_VERSION {
        return None;
    }
    if document.network_fingerprint != fingerprint_road_network(network) {
        return None;
    }
    let tiles = document
        .tiles
        .into_iter()
        .map(|(coord, tile)| (ChunkId::new(coord), tile))
        .collect();
    Some(RoadDeformationStore {
        bake_version: document.bake_version,
        network_fingerprint: document.network_fingerprint,
        revision: 1,
        tiles,
    })
}

pub fn ensure_road_deformation_store(
    world_package_dir: impl AsRef<Path>,
    network: &RoadNetwork,
) -> RoadDeformationStore {
    load_road_deformation_bake(world_package_dir, network)
        .unwrap_or_else(RoadDeformationStore::default)
}
