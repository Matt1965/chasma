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
    DuplicateRegionKey {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
        region_key: String,
    },
    FloorKeyMissing {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
    },
    RegionMissing {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
        region_key: String,
    },
    AmbiguousFloorGeometry {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
    },
    FloorHasNoRegions {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
    },
    RegionReferenceAmbiguous {
        blueprint_id: BuildingNavigationBlueprintId,
        feature_key: String,
    },
    LegacyOutlinePresent {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
    },
    PolygonTooFewVertices {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
        region_key: String,
    },
    PolygonDegenerate {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
        region_key: String,
    },
    RegionAreaTooSmall {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
        region_key: String,
    },
    InvalidRadius {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
    },
    ConnectionSameRegion {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
    },
    ConnectionEndpointOutsideRegion {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
        endpoint: &'static str,
    },
    ConnectionEndpointInOtherRegion {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
        endpoint: &'static str,
    },
    OpenArchWithDoorKey {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
    },
    EntranceRegionAmbiguous {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
    },
    EntranceSpawnOutsideRegion {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
    },
    TransitionRegionAmbiguous {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
        side: &'static str,
    },
    TransitionSameFloor {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
    },
    TransitionOutsideRegion {
        blueprint_id: BuildingNavigationBlueprintId,
        key: String,
        side: &'static str,
    },
    MultiRegionRuntimeUnsupported {
        blueprint_id: BuildingNavigationBlueprintId,
        floor_key: String,
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
            Self::InvalidSchemaVersion {
                blueprint_id,
                version,
            } => write!(
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
            Self::DuplicateRegionKey {
                blueprint_id,
                floor_key,
                region_key,
            } => write!(
                f,
                "duplicate region key `{region_key}` on floor `{floor_key}` in blueprint `{blueprint_id}`"
            ),
            Self::FloorKeyMissing {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "floor key `{floor_key}` missing in blueprint `{blueprint_id}`"
            ),
            Self::RegionMissing {
                blueprint_id,
                floor_key,
                region_key,
            } => write!(
                f,
                "region key `{region_key}` missing on floor `{floor_key}` in blueprint `{blueprint_id}`"
            ),
            Self::AmbiguousFloorGeometry {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "floor `{floor_key}` in blueprint `{blueprint_id}` has both legacy outline and regions"
            ),
            Self::FloorHasNoRegions {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "floor `{floor_key}` in blueprint `{blueprint_id}` has no regions"
            ),
            Self::RegionReferenceAmbiguous {
                blueprint_id,
                feature_key,
            } => write!(
                f,
                "feature `{feature_key}` in blueprint `{blueprint_id}` requires an explicit region key on a multi-region floor"
            ),
            Self::LegacyOutlinePresent {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "floor `{floor_key}` in blueprint `{blueprint_id}` still has legacy walkable outline data"
            ),
            Self::PolygonTooFewVertices {
                blueprint_id,
                floor_key,
                region_key,
            } => write!(
                f,
                "region `{region_key}` on floor `{floor_key}` in blueprint `{blueprint_id}` needs at least three outline vertices"
            ),
            Self::PolygonDegenerate {
                blueprint_id,
                floor_key,
                region_key,
            } => write!(
                f,
                "region `{region_key}` on floor `{floor_key}` in blueprint `{blueprint_id}` has degenerate walkable outline"
            ),
            Self::RegionAreaTooSmall {
                blueprint_id,
                floor_key,
                region_key,
            } => write!(
                f,
                "region `{region_key}` on floor `{floor_key}` in blueprint `{blueprint_id}` has area below minimum"
            ),
            Self::InvalidRadius { blueprint_id, key } => write!(
                f,
                "navigation feature `{key}` in blueprint `{blueprint_id}` has invalid radius"
            ),
            Self::ConnectionSameRegion { blueprint_id, key } => write!(
                f,
                "region connection `{key}` in blueprint `{blueprint_id}` references the same source and destination region"
            ),
            Self::ConnectionEndpointOutsideRegion {
                blueprint_id,
                key,
                endpoint,
            } => write!(
                f,
                "region connection `{key}` in blueprint `{blueprint_id}` has {endpoint} outside its region"
            ),
            Self::ConnectionEndpointInOtherRegion {
                blueprint_id,
                key,
                endpoint,
            } => write!(
                f,
                "region connection `{key}` in blueprint `{blueprint_id}` has {endpoint} inside an unrelated region"
            ),
            Self::OpenArchWithDoorKey { blueprint_id, key } => write!(
                f,
                "open-arch connection `{key}` in blueprint `{blueprint_id}` cannot have a door key"
            ),
            Self::EntranceRegionAmbiguous { blueprint_id, key } => write!(
                f,
                "entrance `{key}` in blueprint `{blueprint_id}` requires an explicit region key on a multi-region floor"
            ),
            Self::EntranceSpawnOutsideRegion { blueprint_id, key } => write!(
                f,
                "entrance `{key}` in blueprint `{blueprint_id}` spawn lies outside its target region"
            ),
            Self::TransitionRegionAmbiguous {
                blueprint_id,
                key,
                side,
            } => write!(
                f,
                "transition `{key}` in blueprint `{blueprint_id}` requires an explicit {side} region key on a multi-region floor"
            ),
            Self::TransitionSameFloor { blueprint_id, key } => write!(
                f,
                "transition `{key}` in blueprint `{blueprint_id}` cannot connect regions on the same floor; use a region connection"
            ),
            Self::TransitionOutsideRegion {
                blueprint_id,
                key,
                side,
            } => write!(
                f,
                "transition `{key}` in blueprint `{blueprint_id}` {side} lies outside its region"
            ),
            Self::MultiRegionRuntimeUnsupported {
                blueprint_id,
                floor_key,
            } => write!(
                f,
                "floor `{floor_key}` in blueprint `{blueprint_id}` has multiple regions; runtime multi-region support is not enabled yet"
            ),
            Self::RonIo(msg) => write!(f, "navigation blueprint catalog io error: {msg}"),
            Self::RonParse(msg) => write!(f, "navigation blueprint catalog parse error: {msg}"),
        }
    }
}

impl std::error::Error for BuildingNavigationBlueprintError {}
