//! Editor-authored archetype persistence (RON).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::building::{BuildingArchetypeCatalog, BuildingArchetypeDefinition};
use super::unit::{UnitArchetypeCatalog, UnitArchetypeDefinition};

pub const UNIT_ARCHETYPES_RON_PATH: &str = "assets/archetypes/unit_archetypes.ron";
pub const BUILDING_ARCHETYPES_RON_PATH: &str = "assets/archetypes/building_archetypes.ron";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchetypePersistenceError {
    Io(String),
    Ron(String),
    Catalog(String),
}

impl std::fmt::Display for ArchetypePersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::Ron(msg) => write!(f, "ron error: {msg}"),
            Self::Catalog(msg) => write!(f, "catalog error: {msg}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UnitArchetypeCatalogRon {
    definitions: Vec<UnitArchetypeDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct BuildingArchetypeCatalogRon {
    definitions: Vec<BuildingArchetypeDefinition>,
}

pub fn load_unit_archetype_catalog_from_ron(
    path: &Path,
) -> Result<UnitArchetypeCatalog, ArchetypePersistenceError> {
    if !path.exists() {
        return Ok(UnitArchetypeCatalog::default());
    }
    let text = fs::read_to_string(path).map_err(|err| ArchetypePersistenceError::Io(err.to_string()))?;
    let parsed: UnitArchetypeCatalogRon =
        ron::from_str(&text).map_err(|err| ArchetypePersistenceError::Ron(err.to_string()))?;
    UnitArchetypeCatalog::from_definitions(parsed.definitions).map_err(|err| {
        ArchetypePersistenceError::Catalog(format!("{err:?}"))
    })
}

pub fn load_building_archetype_catalog_from_ron(
    path: &Path,
) -> Result<BuildingArchetypeCatalog, ArchetypePersistenceError> {
    if !path.exists() {
        return Ok(BuildingArchetypeCatalog::default());
    }
    let text = fs::read_to_string(path).map_err(|err| ArchetypePersistenceError::Io(err.to_string()))?;
    let parsed: BuildingArchetypeCatalogRon =
        ron::from_str(&text).map_err(|err| ArchetypePersistenceError::Ron(err.to_string()))?;
    BuildingArchetypeCatalog::from_definitions(parsed.definitions).map_err(|err| {
        ArchetypePersistenceError::Catalog(format!("{err:?}"))
    })
}

pub fn save_unit_archetype_catalog_to_ron(
    catalog: &UnitArchetypeCatalog,
    path: &Path,
) -> Result<PathBuf, ArchetypePersistenceError> {
    write_catalog_ron(
        path,
        &UnitArchetypeCatalogRon {
            definitions: catalog.definitions().to_vec(),
        },
    )
}

pub fn save_building_archetype_catalog_to_ron(
    catalog: &BuildingArchetypeCatalog,
    path: &Path,
) -> Result<PathBuf, ArchetypePersistenceError> {
    write_catalog_ron(
        path,
        &BuildingArchetypeCatalogRon {
            definitions: catalog.definitions().to_vec(),
        },
    )
}

fn write_catalog_ron<T: Serialize>(path: &Path, value: &T) -> Result<PathBuf, ArchetypePersistenceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| ArchetypePersistenceError::Io(err.to_string()))?;
    }
    let text = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())
        .map_err(|err| ArchetypePersistenceError::Ron(err.to_string()))?;
    let tmp = path.with_extension("ron.tmp");
    fs::write(&tmp, text).map_err(|err| ArchetypePersistenceError::Io(err.to_string()))?;
    fs::rename(&tmp, path).map_err(|err| ArchetypePersistenceError::Io(err.to_string()))?;
    Ok(path.to_path_buf())
}

pub fn load_dev_unit_archetype_catalog() -> UnitArchetypeCatalog {
    load_unit_archetype_catalog_from_ron(Path::new(UNIT_ARCHETYPES_RON_PATH))
        .unwrap_or_else(|err| {
            crate::logging::append_log_line(
                crate::logging::DEV_STARTUP_LOG_PATH,
                "dev startup",
                &format!(
                    "Unit archetype RON load failed ({}): {err}; using empty catalog",
                    UNIT_ARCHETYPES_RON_PATH
                ),
            );
            UnitArchetypeCatalog::default()
        })
}

pub fn load_dev_building_archetype_catalog() -> BuildingArchetypeCatalog {
    load_building_archetype_catalog_from_ron(Path::new(BUILDING_ARCHETYPES_RON_PATH))
        .unwrap_or_else(|err| {
            crate::logging::append_log_line(
                crate::logging::DEV_STARTUP_LOG_PATH,
                "dev startup",
                &format!(
                    "Building archetype RON load failed ({}): {err}; using empty catalog",
                    BUILDING_ARCHETYPES_RON_PATH
                ),
            );
            BuildingArchetypeCatalog::default()
        })
}
