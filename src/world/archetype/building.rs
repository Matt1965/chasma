use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::building::BuildingLifecycleState;
use crate::world::ownership::{OwnerId, TeamId};
use crate::world::{Affiliation, BuildingDefinitionId};

/// Default capture margin when authoring a new building archetype.
pub const DEFAULT_BUILDING_ARCHETYPE_CAPTURE_MARGIN_METERS: f32 = 3.0;

/// Stable key for a building spawn archetype preset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct BuildingArchetypeId(pub String);

impl BuildingArchetypeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Durable authored building configuration saved from a dev editor instance.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BuildingArchetypeSnapshot {
    pub affiliation: Affiliation,
    pub team_id: Option<TeamId>,
    pub owner_id: Option<OwnerId>,
    pub lifecycle_state: BuildingLifecycleState,
    pub container_locked: bool,
    pub uniform_scale: f32,
    pub placement_yaw_deg: f32,
}

/// Authoring metadata for spatial capture (recapture convenience only).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BuildingArchetypeCaptureMetadata {
    pub capture_margin_meters: f32,
}

impl Default for BuildingArchetypeCaptureMetadata {
    fn default() -> Self {
        Self {
            capture_margin_meters: DEFAULT_BUILDING_ARCHETYPE_CAPTURE_MARGIN_METERS,
        }
    }
}

/// Kind of spatial member captured relative to a root building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize)]
pub enum BuildingArchetypeMemberKind {
    Building,
    Doodad,
}

/// Pose of a captured member relative to the root building anchor.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BuildingArchetypeLocalPose {
    pub local_position: [f32; 3],
    pub local_rotation: [f32; 4],
    /// Uniform scale milli for building members (`1000` = 1.0). Zero for doodad members.
    #[serde(default)]
    pub uniform_scale_milli: i32,
    #[serde(default)]
    pub scale_x_milli: i32,
    #[serde(default)]
    pub scale_y_milli: i32,
    #[serde(default)]
    pub scale_z_milli: i32,
}

/// One spatially captured world object stored in a building archetype template.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BuildingArchetypeMember {
    pub kind: BuildingArchetypeMemberKind,
    pub definition_id: String,
    pub local_pose: BuildingArchetypeLocalPose,
}

/// Editor-authored building template associated with one base building type.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BuildingArchetypeDefinition {
    pub id: BuildingArchetypeId,
    pub display_name: String,
    pub base_building_id: BuildingDefinitionId,
    pub snapshot: BuildingArchetypeSnapshot,
    #[serde(default)]
    pub capture_metadata: BuildingArchetypeCaptureMetadata,
    #[serde(default)]
    pub members: Vec<BuildingArchetypeMember>,
    pub enabled: bool,
}

impl BuildingArchetypeDefinition {
    pub fn applies_to(&self, building_id: &BuildingDefinitionId) -> bool {
        &self.base_building_id == building_id
    }
}

/// Read-only registry of building archetype presets.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct BuildingArchetypeCatalog {
    definitions: Vec<BuildingArchetypeDefinition>,
    by_id: HashMap<BuildingArchetypeId, usize>,
}

impl Default for BuildingArchetypeCatalog {
    fn default() -> Self {
        Self {
            definitions: Vec::new(),
            by_id: HashMap::new(),
        }
    }
}

impl BuildingArchetypeCatalog {
    pub fn from_definitions(
        definitions: Vec<BuildingArchetypeDefinition>,
    ) -> Result<Self, BuildingArchetypeCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(BuildingArchetypeCatalogError::DuplicateId(definition.id.clone()));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn definitions(&self) -> &[BuildingArchetypeDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &BuildingArchetypeId) -> Option<&BuildingArchetypeDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn upsert(
        &mut self,
        definition: BuildingArchetypeDefinition,
    ) -> Result<(), BuildingArchetypeCatalogError> {
        if let Some(&index) = self.by_id.get(&definition.id) {
            self.definitions[index] = definition;
            return Ok(());
        }
        let index = self.definitions.len();
        self.by_id.insert(definition.id.clone(), index);
        self.definitions.push(definition);
        Ok(())
    }

    pub fn remove(&mut self, id: &BuildingArchetypeId) -> bool {
        let Some(index) = self.by_id.remove(id) else {
            return false;
        };
        self.definitions.remove(index);
        self.by_id.clear();
        for (new_index, definition) in self.definitions.iter().enumerate() {
            self.by_id.insert(definition.id.clone(), new_index);
        }
        true
    }

    pub fn archetypes_for_building(
        &self,
        building_id: &BuildingDefinitionId,
        enabled_only: bool,
    ) -> Vec<&BuildingArchetypeDefinition> {
        self.definitions
            .iter()
            .filter(|definition| {
                (!enabled_only || definition.enabled)
                    && definition.applies_to(building_id)
            })
            .collect()
    }

    pub fn display_name_taken(
        &self,
        display_name: &str,
        except_id: Option<&BuildingArchetypeId>,
    ) -> bool {
        let normalized = display_name.trim().to_ascii_lowercase();
        self.definitions.iter().any(|definition| {
            if except_id.is_some_and(|id| id == &definition.id) {
                return false;
            }
            definition.display_name.trim().to_ascii_lowercase() == normalized
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingArchetypeCatalogError {
    DuplicateId(BuildingArchetypeId),
}

pub fn unique_building_archetype_id(
    display_name: &str,
    catalog: &BuildingArchetypeCatalog,
) -> BuildingArchetypeId {
    let base = super::unit::slugify_archetype_id(display_name);
    if catalog.get(&BuildingArchetypeId::new(&base)).is_none() {
        return BuildingArchetypeId::new(base);
    }
    for suffix in 2..10_000 {
        let candidate = format!("{base}_{suffix}");
        if catalog.get(&BuildingArchetypeId::new(&candidate)).is_none() {
            return BuildingArchetypeId::new(candidate);
        }
    }
    BuildingArchetypeId::new(format!(
        "{base}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ))
}
