use bevy::prelude::*;

use crate::world::TaskType;
use crate::world::building::state::BuildingLifecycleState;

/// Data-driven building capability flags (ADR-085 B8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub struct BuildingCapabilities {
    pub construction_site: bool,
    pub workstation: bool,
    pub door_control: bool,
}

/// Authored interaction point relative to building anchor (ADR-085 B8).
#[derive(Debug, Clone, PartialEq)]
pub struct InteractionPointDefinition {
    pub key: &'static str,
    pub local_position: Vec3,
    pub local_facing: Quat,
    pub capacity: u32,
    pub task_type: TaskType,
    pub enabled_states: &'static [BuildingLifecycleState],
}

impl InteractionPointDefinition {
    pub fn enabled_for(&self, state: BuildingLifecycleState) -> bool {
        self.enabled_states.contains(&state)
    }
}

/// Interaction profile for one building type (ADR-085 B8).
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingInteractionProfile {
    pub id: String,
    pub capabilities: BuildingCapabilities,
    pub points: Vec<InteractionPointDefinition>,
}

impl BuildingInteractionProfile {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            capabilities: BuildingCapabilities::default(),
            points: Vec::new(),
        }
    }

    pub fn with_capabilities(mut self, capabilities: BuildingCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_points(mut self, points: Vec<InteractionPointDefinition>) -> Self {
        self.points = points;
        self
    }

    pub fn points_for_state(
        &self,
        state: BuildingLifecycleState,
    ) -> Vec<&InteractionPointDefinition> {
        self.points
            .iter()
            .filter(|point| point.enabled_for(state))
            .collect()
    }
}

/// Catalog of building interaction profiles.
#[derive(Debug, Clone, PartialEq, Resource)]
pub struct BuildingInteractionProfileCatalog {
    profiles: std::collections::BTreeMap<String, BuildingInteractionProfile>,
}

impl BuildingInteractionProfileCatalog {
    pub fn new() -> Self {
        Self {
            profiles: std::collections::BTreeMap::new(),
        }
    }

    pub fn from_profiles(profiles: impl IntoIterator<Item = BuildingInteractionProfile>) -> Self {
        let mut catalog = Self::new();
        for profile in profiles {
            catalog.insert(profile);
        }
        catalog
    }

    pub fn insert(&mut self, profile: BuildingInteractionProfile) {
        self.profiles.insert(profile.id.clone(), profile);
    }

    pub fn get(&self, id: &str) -> Option<&BuildingInteractionProfile> {
        self.profiles.get(id)
    }

    pub fn profile_for_definition(
        &self,
        definition: &crate::world::BuildingDefinition,
    ) -> Option<&BuildingInteractionProfile> {
        if let Some(id) = definition.interaction_profile_id.as_deref() {
            return self.get(id);
        }
        self.get(definition.id.as_str())
    }
}

impl Default for BuildingInteractionProfileCatalog {
    fn default() -> Self {
        Self::from_profiles(starter_interaction_profiles())
    }
}

#[cfg(any(test, feature = "dev"))]
pub fn starter_interaction_profiles() -> Vec<BuildingInteractionProfile> {
    use crate::world::TaskType;

    let construction_states = &[
        BuildingLifecycleState::Planned,
        BuildingLifecycleState::Foundation,
        BuildingLifecycleState::InProgress,
    ];
    let complete = &[BuildingLifecycleState::Complete];

    vec![
        BuildingInteractionProfile::new("hut")
            .with_capabilities(BuildingCapabilities {
                construction_site: true,
                ..Default::default()
            })
            .with_points(vec![
                InteractionPointDefinition {
                    key: "construction_front",
                    local_position: Vec3::new(0.0, 0.0, 2.5),
                    local_facing: Quat::IDENTITY,
                    capacity: 1,
                    task_type: TaskType::ConstructBuilding,
                    enabled_states: construction_states,
                },
                InteractionPointDefinition {
                    key: "construction_side",
                    local_position: Vec3::new(2.5, 0.0, 0.0),
                    local_facing: Quat::IDENTITY,
                    capacity: 1,
                    task_type: TaskType::ConstructBuilding,
                    enabled_states: construction_states,
                },
            ]),
        BuildingInteractionProfile::new("workbench")
            .with_capabilities(BuildingCapabilities {
                workstation: true,
                ..Default::default()
            })
            .with_points(vec![InteractionPointDefinition {
                key: "operate",
                local_position: Vec3::new(0.0, 0.0, 0.8),
                local_facing: Quat::IDENTITY,
                capacity: 1,
                task_type: TaskType::OperateWorkstation,
                enabled_states: complete,
            }]),
    ]
}

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_interaction_profiles() -> Vec<BuildingInteractionProfile> {
    Vec::new()
}

/// Resolve a local interaction point to world coordinates.
pub fn interaction_point_world_position(
    building: &crate::world::BuildingRecord,
    layout: crate::world::ChunkLayout,
    point: &InteractionPointDefinition,
) -> crate::world::WorldPosition {
    let anchor = building.placement.position.to_global(layout);
    let global = anchor + building.placement.rotation * point.local_position;
    crate::world::WorldPosition::from_global(global, layout)
}

/// Maximum distance to apply labor at an interaction point (meters).
pub const INTERACTION_WORK_RANGE_METERS: f32 = 1.75;
