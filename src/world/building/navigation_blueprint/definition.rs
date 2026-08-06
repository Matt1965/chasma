use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::error::BuildingNavigationBlueprintError;
use super::id::{BuildingNavigationBlueprintId, validate_navigation_blueprint_id};

/// Current on-disk schema version for [`BuildingNavigationBlueprint`].
pub const BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION: u32 = 2;

/// Minimum absolute signed area for a valid region polygon (m²).
pub const MIN_REGION_AREA: f32 = 0.5;

/// Minimum radius for a region connection (meters).
pub const MIN_CONNECTION_RADIUS: f32 = 0.25;

/// Closed polygon in building-local XZ meters (NV1.1).
///
/// Vertices are wound counter-clockwise when viewed from above. The polygon is
/// implicitly closed (first vertex is not repeated).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NavigationPolygon2d {
    pub vertices_xz: Vec<[f32; 2]>,
}

impl NavigationPolygon2d {
    pub fn rectangle(width_meters: f32, depth_meters: f32) -> Self {
        Self {
            vertices_xz: vec![
                [0.0, 0.0],
                [width_meters, 0.0],
                [width_meters, depth_meters],
                [0.0, depth_meters],
            ],
        }
    }

    pub fn signed_area(&self) -> f32 {
        let n = self.vertices_xz.len();
        if n < 3 {
            return 0.0;
        }
        let mut area = 0.0_f32;
        for i in 0..n {
            let [x0, z0] = self.vertices_xz[i];
            let [x1, z1] = self.vertices_xz[(i + 1) % n];
            area += x0 * z1 - x1 * z0;
        }
        area * 0.5
    }

    pub(crate) fn validate_region(
        &self,
        blueprint_id: &BuildingNavigationBlueprintId,
        floor_key: &str,
        region_key: &str,
    ) -> Result<(), BuildingNavigationBlueprintError> {
        if region_key.is_empty() {
            return Err(BuildingNavigationBlueprintError::PolygonDegenerate {
                blueprint_id: blueprint_id.clone(),
                floor_key: floor_key.to_string(),
                region_key: region_key.to_string(),
            });
        }
        if self.vertices_xz.len() < 3 {
            return Err(BuildingNavigationBlueprintError::PolygonTooFewVertices {
                blueprint_id: blueprint_id.clone(),
                floor_key: floor_key.to_string(),
                region_key: region_key.to_string(),
            });
        }
        for [x, z] in &self.vertices_xz {
            if !x.is_finite() || !z.is_finite() {
                return Err(BuildingNavigationBlueprintError::PolygonDegenerate {
                    blueprint_id: blueprint_id.clone(),
                    floor_key: floor_key.to_string(),
                    region_key: region_key.to_string(),
                });
            }
        }
        if self.signed_area().abs() <= MIN_REGION_AREA {
            return Err(BuildingNavigationBlueprintError::RegionAreaTooSmall {
                blueprint_id: blueprint_id.clone(),
                floor_key: floor_key.to_string(),
                region_key: region_key.to_string(),
            });
        }
        Ok(())
    }
}

/// One walkable region on a floor (room, corridor, landing).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NavigationRegionDefinition {
    /// Stable key, unique within the owning floor.
    pub key: String,
    pub display_label: String,
    #[serde(default)]
    pub room_tag: Option<String>,
    pub walkable_outline: NavigationPolygon2d,
}

/// One navigable floor inside a building (sparse floor ids supported).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NavigationFloorDefinition {
    /// Sparse floor index (e.g. -1, 0, 2). Intermediate ids may be absent.
    pub floor_id: i32,
    /// Stable string key referenced by entrances and vertical transitions.
    pub key: String,
    pub display_label: String,
    /// Building-local elevation in meters (Y). Scales with instance uniform scale.
    pub elevation_meters: f32,
    /// Visibility grouping for interior camera culling (ADR-083).
    pub visibility_group_id: u32,
    #[serde(default)]
    pub room_tag: Option<String>,

    /// Schema-v1 compatibility only.
    #[serde(
        rename = "walkable_outline",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_walkable_outline_legacy"
    )]
    pub walkable_outline_legacy: Option<NavigationPolygon2d>,

    /// Canonical schema-v2 geometry.
    #[serde(default)]
    pub regions: Vec<NavigationRegionDefinition>,
}

impl NavigationFloorDefinition {
    /// The sole region when this floor has exactly one region.
    pub fn sole_region(&self) -> Option<&NavigationRegionDefinition> {
        if self.regions.len() == 1 {
            self.regions.first()
        } else {
            None
        }
    }

    /// Mutable access to the sole region when this floor has exactly one region.
    pub fn sole_region_mut(&mut self) -> Option<&mut NavigationRegionDefinition> {
        if self.regions.len() == 1 {
            self.regions.first_mut()
        } else {
            None
        }
    }

    /// Outline of the sole region for temporary single-region runtime compatibility.
    pub fn sole_region_outline(&self) -> Option<&NavigationPolygon2d> {
        self.sole_region().map(|region| &region.walkable_outline)
    }

    /// Mutable outline of the sole region for temporary editor compatibility.
    pub fn sole_region_outline_mut(&mut self) -> Option<&mut NavigationPolygon2d> {
        self.sole_region_mut()
            .map(|region| &mut region.walkable_outline)
    }

    pub fn region_by_key(&self, key: &str) -> Option<&NavigationRegionDefinition> {
        self.regions.iter().find(|region| region.key == key)
    }

    pub fn region_by_key_mut(&mut self, key: &str) -> Option<&mut NavigationRegionDefinition> {
        self.regions.iter_mut().find(|region| region.key == key)
    }

    pub fn single_region_key(&self) -> Option<&str> {
        if self.regions.len() == 1 {
            Some(self.regions[0].key.as_str())
        } else {
            None
        }
    }
}

/// Exterior entrance from surface into a building floor.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NavigationEntranceDefinition {
    pub key: String,
    pub floor_key: String,
    #[serde(default)]
    pub region_key: Option<String>,
    /// Portal center on the building exterior in local XZ.
    pub local_position_xz: [f32; 2],
    pub radius_meters: f32,
    /// Spawn position after entering, in building-local XYZ.
    pub interior_spawn_local: [f32; 3],
    #[serde(default = "default_true")]
    pub bidirectional: bool,
    /// Interior-profile door key that controls this entrance, when one does.
    ///
    /// `None` means the entrance is doorless and stays enabled. Mirrors
    /// [`NavigationRegionConnectionDefinition::door_key`].
    #[serde(default)]
    pub door_key: Option<String>,
}

fn default_true() -> bool {
    true
}

fn deserialize_walkable_outline_legacy<'de, D>(
    deserializer: D,
) -> Result<Option<NavigationPolygon2d>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum WalkableOutlineLegacyRon {
        Bare(NavigationPolygon2d),
        Optional(Option<NavigationPolygon2d>),
    }

    match WalkableOutlineLegacyRon::deserialize(deserializer)? {
        WalkableOutlineLegacyRon::Bare(polygon) => Ok(Some(polygon)),
        WalkableOutlineLegacyRon::Optional(value) => Ok(value),
    }
}

/// Vertical movement between two authored floors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum NavigationVerticalTransitionKind {
    Stair,
    Ramp,
    /// Reserved for future pathfinding; not consumed by runtime yet.
    Ladder,
}

/// Stairs, ramps, or future ladders between interior floors.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NavigationVerticalTransitionDefinition {
    pub key: String,
    pub kind: NavigationVerticalTransitionKind,
    pub from_floor_key: String,
    pub to_floor_key: String,
    #[serde(default)]
    pub from_region_key: Option<String>,
    #[serde(default)]
    pub to_region_key: Option<String>,
    pub from_local_position_xz: [f32; 2],
    pub from_radius_meters: f32,
    pub to_local_position: [f32; 3],
    #[serde(default = "default_true")]
    pub bidirectional: bool,
}

/// Same-floor passage between two regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum NavigationRegionConnectionKind {
    Doorway,
    OpenArch,
}

/// Legal passage between two walkable regions on the same floor.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NavigationRegionConnectionDefinition {
    /// Unique across all blueprint feature keys.
    pub key: String,
    pub kind: NavigationRegionConnectionKind,
    pub floor_key: String,
    pub from_region_key: String,
    pub to_region_key: String,
    pub from_local_position_xz: [f32; 2],
    pub to_local_position_xz: [f32; 2],
    pub radius_meters: f32,
    #[serde(default = "default_true")]
    pub bidirectional: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub door_key: Option<String>,
}

/// Optional authoring metadata and future pipeline hooks.
#[derive(Debug, Clone, PartialEq, Default, Reflect, Serialize, Deserialize)]
pub struct BuildingNavigationBlueprintMetadata {
    /// Source GLB render key used by future auto-generation (NV1.2+).
    #[serde(default)]
    pub source_render_key: Option<String>,
    /// Monotonic revision from future generator runs.
    #[serde(default)]
    pub generation_revision: Option<u32>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Free-form extension map for tooling without schema churn.
    #[serde(default)]
    pub extensions: std::collections::BTreeMap<String, String>,
}

/// Authoritative navigation description for a building type (NV1.1+).
///
/// All coordinates are building-local. World placement composes via
/// [`BuildingPlacement`] and asset transform standardization (ADR-126–129).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BuildingNavigationBlueprint {
    pub id: BuildingNavigationBlueprintId,
    pub display_name: String,
    pub schema_version: u32,
    #[serde(default)]
    pub metadata: BuildingNavigationBlueprintMetadata,
    pub floors: Vec<NavigationFloorDefinition>,
    pub entrances: Vec<NavigationEntranceDefinition>,
    pub vertical_transitions: Vec<NavigationVerticalTransitionDefinition>,
    #[serde(default)]
    pub region_connections: Vec<NavigationRegionConnectionDefinition>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl BuildingNavigationBlueprint {
    pub fn new(
        id: impl Into<BuildingNavigationBlueprintId>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            schema_version: BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION,
            metadata: BuildingNavigationBlueprintMetadata::default(),
            floors: Vec::new(),
            entrances: Vec::new(),
            vertical_transitions: Vec::new(),
            region_connections: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_floors(mut self, floors: Vec<NavigationFloorDefinition>) -> Self {
        self.floors = floors;
        self
    }

    pub fn with_entrances(mut self, entrances: Vec<NavigationEntranceDefinition>) -> Self {
        self.entrances = entrances;
        self
    }

    pub fn with_vertical_transitions(
        mut self,
        transitions: Vec<NavigationVerticalTransitionDefinition>,
    ) -> Self {
        self.vertical_transitions = transitions;
        self
    }

    pub fn with_region_connections(
        mut self,
        connections: Vec<NavigationRegionConnectionDefinition>,
    ) -> Self {
        self.region_connections = connections;
        self
    }

    pub fn floor_by_key(&self, key: &str) -> Option<&NavigationFloorDefinition> {
        self.floors.iter().find(|floor| floor.key == key)
    }

    /// Whether this blueprint has multi-region topology that generator regen cannot preserve.
    pub fn has_authored_multi_region_topology(&self) -> bool {
        self.floors.iter().any(|floor| floor.regions.len() > 1)
            || !self.region_connections.is_empty()
    }

    /// Whether accepting a generated draft would replace non-trivial authored navigation topology.
    pub fn regeneration_would_replace_authored_topology(&self) -> bool {
        self.has_authored_multi_region_topology()
            || self
                .entrances
                .iter()
                .any(|entrance| entrance.region_key.is_some())
            || self.vertical_transitions.iter().any(|transition| {
                transition.from_region_key.is_some() || transition.to_region_key.is_some()
            })
    }

    pub fn resolve_region_key<'a>(
        &'a self,
        floor_key: &str,
        region_key: Option<&str>,
        feature_key: &str,
    ) -> Result<&'a str, BuildingNavigationBlueprintError> {
        let floor = self.floor_by_key(floor_key).ok_or_else(|| {
            BuildingNavigationBlueprintError::FloorKeyMissing {
                blueprint_id: self.id.clone(),
                floor_key: floor_key.to_string(),
            }
        })?;
        if let Some(key) = region_key {
            let region = floor.region_by_key(key).ok_or_else(|| {
                BuildingNavigationBlueprintError::RegionMissing {
                    blueprint_id: self.id.clone(),
                    floor_key: floor_key.to_string(),
                    region_key: key.to_string(),
                }
            })?;
            Ok(region.key.as_str())
        } else if let Some(key) = floor.single_region_key() {
            Ok(key)
        } else {
            Err(BuildingNavigationBlueprintError::RegionReferenceAmbiguous {
                blueprint_id: self.id.clone(),
                feature_key: feature_key.to_string(),
            })
        }
    }

    /// Reject multi-region blueprints until IN-07b runtime consumption lands.
    pub fn ensure_runtime_single_region_compatible(
        &self,
    ) -> Result<(), BuildingNavigationBlueprintError> {
        for floor in &self.floors {
            if floor.regions.len() != 1 {
                return Err(
                    BuildingNavigationBlueprintError::MultiRegionRuntimeUnsupported {
                        blueprint_id: self.id.clone(),
                        floor_key: floor.key.clone(),
                    },
                );
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), BuildingNavigationBlueprintError> {
        validate_navigation_blueprint_id(self.id.as_str())
            .map_err(BuildingNavigationBlueprintError::InvalidBlueprintId)?;
        if self.schema_version != BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION {
            return Err(BuildingNavigationBlueprintError::InvalidSchemaVersion {
                blueprint_id: self.id.clone(),
                version: self.schema_version,
            });
        }

        let mut floor_keys = std::collections::BTreeSet::new();
        let mut floor_ids = std::collections::BTreeSet::new();
        for floor in &self.floors {
            if !floor_keys.insert(floor.key.clone()) {
                return Err(BuildingNavigationBlueprintError::DuplicateFloorKey {
                    blueprint_id: self.id.clone(),
                    floor_key: floor.key.clone(),
                });
            }
            if !floor_ids.insert(floor.floor_id) {
                return Err(BuildingNavigationBlueprintError::DuplicateFloorId {
                    blueprint_id: self.id.clone(),
                    floor_id: floor.floor_id,
                });
            }
            if !floor.elevation_meters.is_finite() {
                return Err(BuildingNavigationBlueprintError::PolygonDegenerate {
                    blueprint_id: self.id.clone(),
                    floor_key: floor.key.clone(),
                    region_key: String::new(),
                });
            }
            if floor.walkable_outline_legacy.is_some() {
                return Err(BuildingNavigationBlueprintError::LegacyOutlinePresent {
                    blueprint_id: self.id.clone(),
                    floor_key: floor.key.clone(),
                });
            }
            if floor.regions.is_empty() {
                return Err(BuildingNavigationBlueprintError::FloorHasNoRegions {
                    blueprint_id: self.id.clone(),
                    floor_key: floor.key.clone(),
                });
            }
            let mut region_keys = std::collections::BTreeSet::new();
            for region in &floor.regions {
                if region.key.is_empty() {
                    return Err(BuildingNavigationBlueprintError::PolygonDegenerate {
                        blueprint_id: self.id.clone(),
                        floor_key: floor.key.clone(),
                        region_key: region.key.clone(),
                    });
                }
                if !region_keys.insert(region.key.clone()) {
                    return Err(BuildingNavigationBlueprintError::DuplicateRegionKey {
                        blueprint_id: self.id.clone(),
                        floor_key: floor.key.clone(),
                        region_key: region.key.clone(),
                    });
                }
                region
                    .walkable_outline
                    .validate_region(&self.id, &floor.key, &region.key)?;
            }
        }

        let mut feature_keys = std::collections::BTreeSet::new();
        for entrance in &self.entrances {
            if !feature_keys.insert(entrance.key.clone()) {
                return Err(BuildingNavigationBlueprintError::DuplicateFeatureKey {
                    blueprint_id: self.id.clone(),
                    key: entrance.key.clone(),
                });
            }
            self.require_floor(&entrance.floor_key)?;
            validate_radius(&self.id, &entrance.key, entrance.radius_meters)?;
            let region_key = self.resolve_region_key(
                &entrance.floor_key,
                entrance.region_key.as_deref(),
                &entrance.key,
            )?;
            let floor = self.floor_by_key(&entrance.floor_key).expect("checked");
            let region = floor.region_by_key(region_key).expect("checked");
            let spawn_xz = Vec2::new(
                entrance.interior_spawn_local[0],
                entrance.interior_spawn_local[2],
            );
            if !point_inside_polygon(&region.walkable_outline.vertices_xz, spawn_xz) {
                return Err(
                    BuildingNavigationBlueprintError::EntranceSpawnOutsideRegion {
                        blueprint_id: self.id.clone(),
                        key: entrance.key.clone(),
                    },
                );
            }
        }
        for transition in &self.vertical_transitions {
            if !feature_keys.insert(transition.key.clone()) {
                return Err(BuildingNavigationBlueprintError::DuplicateFeatureKey {
                    blueprint_id: self.id.clone(),
                    key: transition.key.clone(),
                });
            }
            if transition.from_floor_key == transition.to_floor_key {
                return Err(BuildingNavigationBlueprintError::TransitionSameFloor {
                    blueprint_id: self.id.clone(),
                    key: transition.key.clone(),
                });
            }
            self.require_floor(&transition.from_floor_key)?;
            self.require_floor(&transition.to_floor_key)?;
            validate_radius(&self.id, &transition.key, transition.from_radius_meters)?;
            let from_region_key = self.resolve_region_key(
                &transition.from_floor_key,
                transition.from_region_key.as_deref(),
                &transition.key,
            )?;
            let to_region_key = self.resolve_region_key(
                &transition.to_floor_key,
                transition.to_region_key.as_deref(),
                &transition.key,
            )?;
            let from_floor = self
                .floor_by_key(&transition.from_floor_key)
                .expect("checked");
            let to_floor = self
                .floor_by_key(&transition.to_floor_key)
                .expect("checked");
            let from_region = from_floor.region_by_key(from_region_key).expect("checked");
            let to_region = to_floor.region_by_key(to_region_key).expect("checked");
            let from_pos = Vec2::new(
                transition.from_local_position_xz[0],
                transition.from_local_position_xz[1],
            );
            if !point_inside_polygon(&from_region.walkable_outline.vertices_xz, from_pos) {
                return Err(BuildingNavigationBlueprintError::TransitionOutsideRegion {
                    blueprint_id: self.id.clone(),
                    key: transition.key.clone(),
                    side: "source",
                });
            }
            let to_pos = Vec2::new(
                transition.to_local_position[0],
                transition.to_local_position[2],
            );
            if !point_inside_polygon(&to_region.walkable_outline.vertices_xz, to_pos) {
                return Err(BuildingNavigationBlueprintError::TransitionOutsideRegion {
                    blueprint_id: self.id.clone(),
                    key: transition.key.clone(),
                    side: "destination",
                });
            }
        }
        for connection in &self.region_connections {
            if !feature_keys.insert(connection.key.clone()) {
                return Err(BuildingNavigationBlueprintError::DuplicateFeatureKey {
                    blueprint_id: self.id.clone(),
                    key: connection.key.clone(),
                });
            }
            self.validate_region_connection(connection)?;
        }
        Ok(())
    }

    fn validate_region_connection(
        &self,
        connection: &NavigationRegionConnectionDefinition,
    ) -> Result<(), BuildingNavigationBlueprintError> {
        self.require_floor(&connection.floor_key)?;
        if connection.from_region_key == connection.to_region_key {
            return Err(BuildingNavigationBlueprintError::ConnectionSameRegion {
                blueprint_id: self.id.clone(),
                key: connection.key.clone(),
            });
        }
        if !(connection.radius_meters > 0.0)
            || !connection.radius_meters.is_finite()
            || connection.radius_meters < MIN_CONNECTION_RADIUS
        {
            return Err(BuildingNavigationBlueprintError::InvalidRadius {
                blueprint_id: self.id.clone(),
                key: connection.key.clone(),
            });
        }
        if connection.kind == NavigationRegionConnectionKind::OpenArch
            && connection.door_key.is_some()
        {
            return Err(BuildingNavigationBlueprintError::OpenArchWithDoorKey {
                blueprint_id: self.id.clone(),
                key: connection.key.clone(),
            });
        }
        let floor = self.floor_by_key(&connection.floor_key).expect("checked");
        let from_region = floor
            .region_by_key(&connection.from_region_key)
            .ok_or_else(|| BuildingNavigationBlueprintError::RegionMissing {
                blueprint_id: self.id.clone(),
                floor_key: connection.floor_key.clone(),
                region_key: connection.from_region_key.clone(),
            })?;
        let to_region = floor
            .region_by_key(&connection.to_region_key)
            .ok_or_else(|| BuildingNavigationBlueprintError::RegionMissing {
                blueprint_id: self.id.clone(),
                floor_key: connection.floor_key.clone(),
                region_key: connection.to_region_key.clone(),
            })?;
        let from_pos = Vec2::new(
            connection.from_local_position_xz[0],
            connection.from_local_position_xz[1],
        );
        let to_pos = Vec2::new(
            connection.to_local_position_xz[0],
            connection.to_local_position_xz[1],
        );
        if !from_pos.x.is_finite()
            || !from_pos.y.is_finite()
            || !to_pos.x.is_finite()
            || !to_pos.y.is_finite()
        {
            return Err(
                BuildingNavigationBlueprintError::ConnectionEndpointOutsideRegion {
                    blueprint_id: self.id.clone(),
                    key: connection.key.clone(),
                    endpoint: "source",
                },
            );
        }
        if !point_inside_polygon(&from_region.walkable_outline.vertices_xz, from_pos) {
            return Err(
                BuildingNavigationBlueprintError::ConnectionEndpointOutsideRegion {
                    blueprint_id: self.id.clone(),
                    key: connection.key.clone(),
                    endpoint: "source",
                },
            );
        }
        if !point_inside_polygon(&to_region.walkable_outline.vertices_xz, to_pos) {
            return Err(
                BuildingNavigationBlueprintError::ConnectionEndpointOutsideRegion {
                    blueprint_id: self.id.clone(),
                    key: connection.key.clone(),
                    endpoint: "destination",
                },
            );
        }
        for region in &floor.regions {
            if region.key == connection.from_region_key || region.key == connection.to_region_key {
                continue;
            }
            if point_inside_polygon(&region.walkable_outline.vertices_xz, from_pos) {
                return Err(
                    BuildingNavigationBlueprintError::ConnectionEndpointInOtherRegion {
                        blueprint_id: self.id.clone(),
                        key: connection.key.clone(),
                        endpoint: "source",
                    },
                );
            }
            if point_inside_polygon(&region.walkable_outline.vertices_xz, to_pos) {
                return Err(
                    BuildingNavigationBlueprintError::ConnectionEndpointInOtherRegion {
                        blueprint_id: self.id.clone(),
                        key: connection.key.clone(),
                        endpoint: "destination",
                    },
                );
            }
        }
        let _ = to_region;
        Ok(())
    }

    fn require_floor(&self, floor_key: &str) -> Result<(), BuildingNavigationBlueprintError> {
        if self.floor_by_key(floor_key).is_some() {
            Ok(())
        } else {
            Err(BuildingNavigationBlueprintError::FloorKeyMissing {
                blueprint_id: self.id.clone(),
                floor_key: floor_key.to_string(),
            })
        }
    }
}

fn validate_radius(
    blueprint_id: &BuildingNavigationBlueprintId,
    key: &str,
    radius_meters: f32,
) -> Result<(), BuildingNavigationBlueprintError> {
    if radius_meters > 0.0 && radius_meters.is_finite() {
        Ok(())
    } else {
        Err(BuildingNavigationBlueprintError::InvalidRadius {
            blueprint_id: blueprint_id.clone(),
            key: key.to_string(),
        })
    }
}

/// Ray-cast point-in-polygon test for building-local XZ coordinates.
pub fn point_inside_polygon(vertices: &[[f32; 2]], point: Vec2) -> bool {
    let mut inside = false;
    let mut j = vertices.len().wrapping_sub(1);
    for (i, [x, z]) in vertices.iter().enumerate() {
        let pi = Vec2::new(*x, *z);
        let pj = Vec2::new(vertices[j][0], vertices[j][1]);
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y + f32::EPSILON) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Build a schema-v2 floor with exactly one region.
pub fn single_region_floor(
    floor_id: i32,
    key: impl Into<String>,
    display_label: impl Into<String>,
    elevation_meters: f32,
    visibility_group_id: u32,
    room_tag: Option<String>,
    outline: NavigationPolygon2d,
) -> NavigationFloorDefinition {
    let key = key.into();
    let display_label = display_label.into();
    NavigationFloorDefinition {
        floor_id,
        key: key.clone(),
        display_label: display_label.clone(),
        elevation_meters,
        visibility_group_id,
        room_tag: room_tag.clone(),
        walkable_outline_legacy: None,
        regions: vec![NavigationRegionDefinition {
            key: "main".to_string(),
            display_label,
            room_tag,
            walkable_outline: outline,
        }],
    }
}

/// Instance-only navigation override (NV1.1).
///
/// Does not modify the asset catalog. An inline blueprint may later be promoted
/// to a catalog variant by assigning [`Self::blueprint_id`].
#[derive(Debug, Clone, PartialEq, Default, Reflect, Serialize, Deserialize)]
pub struct BuildingNavigationBlueprintInstanceOverride {
    /// Reference to an alternate catalog blueprint (variant promotion seam).
    #[serde(default)]
    pub blueprint_id: Option<BuildingNavigationBlueprintId>,
    /// Full inline blueprint for this instance only.
    #[serde(default)]
    pub inline_blueprint: Option<BuildingNavigationBlueprint>,
}

impl BuildingNavigationBlueprintInstanceOverride {
    pub fn catalog(blueprint_id: impl Into<BuildingNavigationBlueprintId>) -> Self {
        Self {
            blueprint_id: Some(blueprint_id.into()),
            inline_blueprint: None,
        }
    }

    pub fn inline(blueprint: BuildingNavigationBlueprint) -> Self {
        Self {
            blueprint_id: None,
            inline_blueprint: Some(blueprint),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_floor(key: &str, floor_id: i32) -> NavigationFloorDefinition {
        single_region_floor(
            floor_id,
            key,
            key,
            floor_id as f32 * 4.0,
            (floor_id + 1) as u32,
            None,
            NavigationPolygon2d::rectangle(4.0, 4.0),
        )
    }

    fn two_region_floor() -> NavigationFloorDefinition {
        NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![
                NavigationRegionDefinition {
                    key: "west".to_string(),
                    display_label: "West".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d::rectangle(5.0, 4.0),
                },
                NavigationRegionDefinition {
                    key: "east".to_string(),
                    display_label: "East".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d {
                        vertices_xz: vec![[5.4, 0.0], [10.4, 0.0], [10.4, 4.0], [5.4, 4.0]],
                    },
                },
            ],
        }
    }

    #[test]
    fn sparse_floor_ids_validate() {
        let blueprint = BuildingNavigationBlueprint::new("sparse_hut", "Sparse Hut")
            .with_floors(vec![
                sample_floor("basement", -1),
                sample_floor("ground", 0),
                sample_floor("attic", 2),
            ])
            .with_entrances(vec![NavigationEntranceDefinition {
                key: "main_door".to_string(),
                floor_key: "ground".to_string(),
                region_key: Some("main".to_string()),
                local_position_xz: [2.0, 0.0],
                radius_meters: 1.5,
                interior_spawn_local: [2.0, 0.0, 1.0],
                bidirectional: true,
                door_key: None,
            }]);
        blueprint.validate().expect("sparse floors should validate");
    }

    #[test]
    fn missing_floor_key_rejected() {
        let blueprint = BuildingNavigationBlueprint::new("bad", "Bad")
            .with_floors(vec![sample_floor("ground", 0)])
            .with_entrances(vec![NavigationEntranceDefinition {
                key: "door".to_string(),
                floor_key: "missing".to_string(),
                region_key: Some("main".to_string()),
                local_position_xz: [0.0, 0.0],
                radius_meters: 1.0,
                interior_spawn_local: [0.0, 0.0, 0.0],
                bidirectional: true,
                door_key: None,
            }]);
        assert!(matches!(
            blueprint.validate(),
            Err(BuildingNavigationBlueprintError::FloorKeyMissing { .. })
        ));
    }

    #[test]
    fn duplicate_region_key_rejected() {
        let mut floor = sample_floor("ground", 0);
        floor.regions.push(NavigationRegionDefinition {
            key: "main".to_string(),
            display_label: "Duplicate".to_string(),
            room_tag: None,
            walkable_outline: NavigationPolygon2d::rectangle(2.0, 2.0),
        });
        let blueprint = BuildingNavigationBlueprint::new("bad", "Bad").with_floors(vec![floor]);
        assert!(matches!(
            blueprint.validate(),
            Err(BuildingNavigationBlueprintError::DuplicateRegionKey { .. })
        ));
    }

    #[test]
    fn empty_region_list_rejected() {
        let mut floor = sample_floor("ground", 0);
        floor.regions.clear();
        let blueprint = BuildingNavigationBlueprint::new("bad", "Bad").with_floors(vec![floor]);
        assert!(matches!(
            blueprint.validate(),
            Err(BuildingNavigationBlueprintError::FloorHasNoRegions { .. })
        ));
    }

    #[test]
    fn connection_same_region_rejected() {
        let blueprint = BuildingNavigationBlueprint::new("bad", "Bad")
            .with_floors(vec![sample_floor("ground", 0)])
            .with_region_connections(vec![NavigationRegionConnectionDefinition {
                key: "door".to_string(),
                kind: NavigationRegionConnectionKind::Doorway,
                floor_key: "ground".to_string(),
                from_region_key: "main".to_string(),
                to_region_key: "main".to_string(),
                from_local_position_xz: [2.0, 2.0],
                to_local_position_xz: [2.5, 2.0],
                radius_meters: 0.8,
                bidirectional: true,
                enabled: true,
                door_key: None,
            }]);
        assert!(matches!(
            blueprint.validate(),
            Err(BuildingNavigationBlueprintError::ConnectionSameRegion { .. })
        ));
    }

    #[test]
    fn valid_two_region_floor_accepted() {
        let blueprint = BuildingNavigationBlueprint::new("split", "Split")
            .with_floors(vec![two_region_floor()])
            .with_region_connections(vec![NavigationRegionConnectionDefinition {
                key: "hall".to_string(),
                kind: NavigationRegionConnectionKind::Doorway,
                floor_key: "ground".to_string(),
                from_region_key: "west".to_string(),
                to_region_key: "east".to_string(),
                from_local_position_xz: [4.7, 2.0],
                to_local_position_xz: [5.7, 2.0],
                radius_meters: 0.8,
                bidirectional: true,
                enabled: true,
                door_key: None,
            }]);
        blueprint
            .validate()
            .expect("two-region floor should validate");
    }

    #[test]
    fn transition_same_floor_rejected() {
        let blueprint = BuildingNavigationBlueprint::new("bad", "Bad")
            .with_floors(vec![sample_floor("ground", 0), sample_floor("upper", 1)])
            .with_vertical_transitions(vec![NavigationVerticalTransitionDefinition {
                key: "bad_stair".to_string(),
                kind: NavigationVerticalTransitionKind::Stair,
                from_floor_key: "ground".to_string(),
                to_floor_key: "ground".to_string(),
                from_region_key: Some("main".to_string()),
                to_region_key: Some("main".to_string()),
                from_local_position_xz: [1.0, 1.0],
                from_radius_meters: 1.0,
                to_local_position: [1.0, 0.0, 1.0],
                bidirectional: true,
            }]);
        assert!(matches!(
            blueprint.validate(),
            Err(BuildingNavigationBlueprintError::TransitionSameFloor { .. })
        ));
    }
}
