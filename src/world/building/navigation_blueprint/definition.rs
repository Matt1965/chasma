use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::error::BuildingNavigationBlueprintError;
use super::id::{BuildingNavigationBlueprintId, validate_navigation_blueprint_id};

/// Current on-disk schema version for [`BuildingNavigationBlueprint`].
pub const BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION: u32 = 1;

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

    fn validate(
        &self,
        blueprint_id: &BuildingNavigationBlueprintId,
        floor_key: &str,
    ) -> Result<(), BuildingNavigationBlueprintError> {
        if self.vertices_xz.len() < 3 {
            return Err(BuildingNavigationBlueprintError::PolygonTooFewVertices {
                blueprint_id: blueprint_id.clone(),
                floor_key: floor_key.to_string(),
            });
        }
        for [x, z] in &self.vertices_xz {
            if !x.is_finite() || !z.is_finite() {
                return Err(BuildingNavigationBlueprintError::PolygonDegenerate {
                    blueprint_id: blueprint_id.clone(),
                    floor_key: floor_key.to_string(),
                });
            }
        }
        if self.signed_area().abs() <= f32::EPSILON {
            return Err(BuildingNavigationBlueprintError::PolygonDegenerate {
                blueprint_id: blueprint_id.clone(),
                floor_key: floor_key.to_string(),
            });
        }
        Ok(())
    }
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
    pub walkable_outline: NavigationPolygon2d,
}

/// Exterior entrance from surface into a building floor.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NavigationEntranceDefinition {
    pub key: String,
    pub floor_key: String,
    /// Portal center on the building exterior in local XZ.
    pub local_position_xz: [f32; 2],
    pub radius_meters: f32,
    /// Spawn position after entering, in building-local XYZ.
    pub interior_spawn_local: [f32; 3],
    #[serde(default = "default_true")]
    pub bidirectional: bool,
}

fn default_true() -> bool {
    true
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
    pub from_local_position_xz: [f32; 2],
    pub from_radius_meters: f32,
    pub to_local_position: [f32; 3],
    #[serde(default = "default_true")]
    pub bidirectional: bool,
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

/// Authoritative navigation description for a building type (NV1.1).
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
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl BuildingNavigationBlueprint {
    pub fn new(id: impl Into<BuildingNavigationBlueprintId>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            schema_version: BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION,
            metadata: BuildingNavigationBlueprintMetadata::default(),
            floors: Vec::new(),
            entrances: Vec::new(),
            vertical_transitions: Vec::new(),
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

    pub fn floor_by_key(&self, key: &str) -> Option<&NavigationFloorDefinition> {
        self.floors.iter().find(|floor| floor.key == key)
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
                });
            }
            floor
                .walkable_outline
                .validate(&self.id, &floor.key)?;
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
        }
        for transition in &self.vertical_transitions {
            if !feature_keys.insert(transition.key.clone()) {
                return Err(BuildingNavigationBlueprintError::DuplicateFeatureKey {
                    blueprint_id: self.id.clone(),
                    key: transition.key.clone(),
                });
            }
            self.require_floor(&transition.from_floor_key)?;
            self.require_floor(&transition.to_floor_key)?;
            validate_radius(&self.id, &transition.key, transition.from_radius_meters)?;
        }
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
        NavigationFloorDefinition {
            floor_id,
            key: key.to_string(),
            display_label: key.to_string(),
            elevation_meters: floor_id as f32 * 4.0,
            visibility_group_id: (floor_id + 1) as u32,
            room_tag: None,
            walkable_outline: NavigationPolygon2d::rectangle(4.0, 4.0),
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
                local_position_xz: [2.0, 0.0],
                radius_meters: 1.5,
                interior_spawn_local: [2.0, 0.0, 1.0],
                bidirectional: true,
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
                local_position_xz: [0.0, 0.0],
                radius_meters: 1.0,
                interior_spawn_local: [0.0, 0.0, 0.0],
                bidirectional: true,
            }]);
        assert!(matches!(
            blueprint.validate(),
            Err(BuildingNavigationBlueprintError::FloorKeyMissing { .. })
        ));
    }
}
