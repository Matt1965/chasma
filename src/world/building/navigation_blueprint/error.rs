use super::id::BuildingNavigationBlueprintId;

/// Catalog and blueprint validation errors (NV1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingNavigationBlueprintError {
    DuplicateId(BuildingNavigationBlueprintId),
    BlueprintMissing(BuildingNavigationBlueprintId),
    BlueprintDisabled(BuildingNavigationBlueprintId),
    InvalidBlueprintId(String),
    InvalidSchemaVersion {
        blueprint_id: BuildingNavigationBlueprintId,
        version: u32,
    },
    DuplicateFloorKey {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
    },
    DuplicateFloorId {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_id: i32,
    },
    DuplicateFeatureKey {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
    },
    FloorKeyMissing {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
    },
    PolygonTooFewVertices {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
    },
    PolygonDegenerate {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
    },
    InvalidRadius {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
    },
    RonIo(String),
    RonParse(String),
}

impl std::fmt::Display for BuildingNavigationBlueprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate navigation blueprint id `{id}`"),
            Self::BlueprintMissing(id) => write!(f, "navigation blueprint `{id}` missing"),
            Self::BlueprintDisabled(id) => write!(f, "navigation blueprint `{id}` disabled"),
            Self::InvalidBlueprintId(id) => write!(f, "invalid navigation blueprint id `{id}`"),
            Self::InvalidSchemaVersion { blueprint_id, version } => write!(
                f,
                "navigation blueprint `{blueprint_id}` schema version {version} unsupported"
            ),
            Self::DuplicateFloorKey {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "duplicate floor key `{floor_key}` in blueprint `{blueprint_id}`"
            ),
            Self::DuplicateFloorId {
                blueprint_id,
                floor_id,
            } => write!(
                f,
                "duplicate floor id {floor_id} in blueprint `{blueprint_id}`"
            ),
            Self::DuplicateFeatureKey { blueprint_id, key } => write!(
                f,
                "duplicate navigation feature key `{key}` in blueprint `{blueprint_id}`"
            ),
            Self::FloorKeyMissing {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "floor key `{floor_key}` missing in blueprint `{blueprint_id}`"
            ),
            Self::PolygonTooFewVertices {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "floor `{floor_key}` in blueprint `{blueprint_id}` needs at least three outline vertices"
            ),
            Self::PolygonDegenerate {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "floor `{floor_key}` in blueprint `{blueprint_id}` has degenerate walkable outline"
            ),
            Self::InvalidRadius {
                blueprint_id,
                key,
            } => write!(
                f,
                "navigation feature `{key}` in blueprint `{blueprint_id}` has invalid radius"
            ),
            Self::RonIo(msg) => write!(f, "navigation blueprint catalog io error: {msg}"),
            Self::RonParse(msg) => write!(f, "navigation blueprint catalog parse error: {msg}"),
        }
    }
}

impl std::error::Error for BuildingNavigationBlueprintError {}
