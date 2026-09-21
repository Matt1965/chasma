use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::warn;
use serde::{Deserialize, Serialize};

use crate::world::{
    AppearanceProfileCatalog, InventoryCatalogCtx, ItemCatalog, UnitCatalog,
};

use super::catalog::OriginCatalog;
use super::definition::OriginDefinition;
use super::starter::seed_origin_catalog;
use super::validation::validate_origin_definition;

pub const ORIGINS_RON_PATH: &str = "assets/worlds/main/origins.ron";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginPersistenceError {
    Io(String),
    Ron(String),
    Catalog(String),
}

impl std::fmt::Display for OriginPersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::Ron(msg) => write!(f, "ron error: {msg}"),
            Self::Catalog(msg) => write!(f, "catalog error: {msg}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct OriginCatalogRon {
    definitions: Vec<OriginDefinition>,
}

pub fn load_origins_from_ron(path: &Path) -> Result<OriginCatalog, OriginPersistenceError> {
    if !path.exists() {
        return Ok(OriginCatalog::default());
    }
    let text = fs::read_to_string(path)
        .map_err(|err| OriginPersistenceError::Io(err.to_string()))?;
    let parsed: OriginCatalogRon =
        ron::from_str(&text).map_err(|err| OriginPersistenceError::Ron(err.to_string()))?;
    OriginCatalog::from_definitions(parsed.definitions)
        .map_err(OriginPersistenceError::Catalog)
}

pub fn save_origins_to_ron(
    catalog: &OriginCatalog,
    path: &Path,
) -> Result<PathBuf, OriginPersistenceError> {
    write_catalog_ron(
        path,
        &OriginCatalogRon {
            definitions: catalog.definitions().to_vec(),
        },
    )
}

fn write_catalog_ron<T: Serialize>(path: &Path, value: &T) -> Result<PathBuf, OriginPersistenceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| OriginPersistenceError::Io(err.to_string()))?;
    }
    let text = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())
        .map_err(|err| OriginPersistenceError::Ron(err.to_string()))?;
    let tmp = path.with_extension("ron.tmp");
    fs::write(&tmp, text).map_err(|err| OriginPersistenceError::Io(err.to_string()))?;
    fs::rename(&tmp, path).map_err(|err| OriginPersistenceError::Io(err.to_string()))?;
    Ok(path.to_path_buf())
}

pub fn load_dev_origin_catalog(
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> OriginCatalog {
    let path = Path::new(ORIGINS_RON_PATH);
    let catalog = if path.exists() {
        load_origins_from_ron(path).unwrap_or_else(|error| {
            warn!("failed to load origins from `{}`: {error}", ORIGINS_RON_PATH);
            seed_origin_catalog(unit_catalog, appearance_profiles)
        })
    } else {
        seed_origin_catalog(unit_catalog, appearance_profiles)
    };
    if validate_loaded_catalog(&catalog, unit_catalog, appearance_profiles, inventory_ctx.items)
        .is_err()
    {
        let seeded = seed_origin_catalog(unit_catalog, appearance_profiles);
        if let Err(error) = save_origins_to_ron(&seeded, path) {
            warn!("failed to seed origins to `{}`: {error}", ORIGINS_RON_PATH);
        }
        return seeded;
    }
    if !path.exists() {
        if let Err(error) = save_origins_to_ron(&catalog, path) {
            warn!("failed to write seeded origins to `{}`: {error}", ORIGINS_RON_PATH);
        }
    }
    catalog
}

fn validate_loaded_catalog(
    catalog: &OriginCatalog,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    item_catalog: &ItemCatalog,
) -> Result<(), String> {
    for origin in catalog.definitions() {
        validate_origin_definition(origin, unit_catalog, appearance_profiles, item_catalog)?;
    }
    Ok(())
}
