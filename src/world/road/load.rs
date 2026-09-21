use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::error::{RoadError, RoadLoadError};
use super::network::{RoadNetwork, RoadNetworkRon, ROAD_NETWORK_SCHEMA_VERSION};
use super::validate::validate_road_network;

/// Default world package directory for the current v1 world scope.
///
/// World-specific path resolution belongs at this persistence boundary only.
pub const DEFAULT_WORLD_PACKAGE_DIR: &str = "assets/worlds/main";

const ROAD_NETWORK_FILE: &str = "roads/network.ron";

pub fn road_network_ron_path(world_package_dir: impl AsRef<Path>) -> PathBuf {
    world_package_dir.as_ref().join(ROAD_NETWORK_FILE)
}

pub fn load_road_network() -> RoadNetwork {
    match load_road_network_from_world_package(DEFAULT_WORLD_PACKAGE_DIR) {
        Ok(network) => network,
        Err(error) => {
            bevy::log::error!("failed to load road network: {error}");
            RoadNetwork::empty()
        }
    }
}

pub fn load_road_network_from_world_package(
    world_package_dir: impl AsRef<Path>,
) -> Result<RoadNetwork, RoadLoadError> {
    let path = road_network_ron_path(world_package_dir);
    load_road_network_from_path(&path)
}

pub fn load_road_network_from_path(path: &Path) -> Result<RoadNetwork, RoadLoadError> {
    if !path.exists() {
        return Ok(RoadNetwork::empty());
    }

    let contents = fs::read_to_string(path)?;
    parse_road_network_ron(&contents)
}

pub fn parse_road_network_ron(contents: &str) -> Result<RoadNetwork, RoadLoadError> {
    let document: RoadNetworkRon = ron::from_str(contents)
        .map_err(|error| RoadLoadError::Parse(error.to_string()))?;
    let network = document.into_network();
    if network.version != ROAD_NETWORK_SCHEMA_VERSION {
        return Err(RoadLoadError::Validation(RoadError::UnsupportedSchemaVersion {
            found: network.version,
            expected: ROAD_NETWORK_SCHEMA_VERSION,
        }));
    }
    validate_road_network(&network)?;
    Ok(network)
}

pub fn serialize_road_network_ron(network: &RoadNetwork) -> Result<String, RoadLoadError> {
    validate_road_network(network)?;
    let document = RoadNetworkRon::from(network.clone());
    ron::ser::to_string_pretty(&document, ron::ser::PrettyConfig::new().new_line("\n".to_string()))
        .map_err(|error| RoadLoadError::Parse(error.to_string()))
}

pub fn save_road_network_to_path(
    path: &Path,
    network: &RoadNetwork,
) -> Result<(), RoadLoadError> {
    let serialized = serialize_road_network_ron(network)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serialized)?;
    Ok(())
}

pub fn save_road_network(network: &RoadNetwork) -> Result<(), RoadLoadError> {
    save_road_network_to_path(
        &road_network_ron_path(DEFAULT_WORLD_PACKAGE_DIR),
        network,
    )
}
