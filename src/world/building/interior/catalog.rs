use bevy::prelude::*;

use super::door::{DoorAccessPolicy, DoorState};
use super::id::InteriorProfileId;
use crate::world::{BuildingDefinitionId, DoodadDefinitionId, PortalTemplate, SpaceTemplate};

/// Interior child object kind (ADR-084 B7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteriorChildKind {
    Doodad(DoodadDefinitionId),
    Building(BuildingDefinitionId),
}

/// Authored interior child placement relative to building anchor.
#[derive(Debug, Clone, PartialEq)]
pub struct InteriorChildPlacement {
    pub key: &'static str,
    pub kind: InteriorChildKind,
    pub space_key: &'static str,
    pub local_position: Vec3,
    pub local_rotation: Quat,
    pub enabled: bool,
}

/// Authored door bound to a portal template key.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorTemplate {
    pub key: &'static str,
    pub portal_key: &'static str,
    pub initial_state: DoorState,
    pub access: DoorAccessPolicy,
}

/// Full interior profile for one building type (ADR-084 B7).
#[derive(Debug, Clone, PartialEq)]
pub struct InteriorProfile {
    pub id: InteriorProfileId,
    pub spaces: Vec<SpaceTemplate>,
    pub portals: Vec<PortalTemplate>,
    pub doors: Vec<DoorTemplate>,
    pub children: Vec<InteriorChildPlacement>,
}

impl InteriorProfile {
    pub fn new(id: InteriorProfileId) -> Self {
        Self {
            id,
            spaces: Vec::new(),
            portals: Vec::new(),
            doors: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_spaces(mut self, spaces: Vec<SpaceTemplate>) -> Self {
        self.spaces = spaces;
        self
    }

    pub fn with_portals(mut self, portals: Vec<PortalTemplate>) -> Self {
        self.portals = portals;
        self
    }

    pub fn with_doors(mut self, doors: Vec<DoorTemplate>) -> Self {
        self.doors = doors;
        self
    }

    pub fn with_children(mut self, children: Vec<InteriorChildPlacement>) -> Self {
        self.children = children;
        self
    }
}

/// Catalog of interior profiles keyed by id.
#[derive(Debug, Clone, PartialEq, Resource)]
pub struct InteriorProfileCatalog {
    profiles: std::collections::BTreeMap<String, InteriorProfile>,
}

impl InteriorProfileCatalog {
    pub fn new() -> Self {
        Self {
            profiles: std::collections::BTreeMap::new(),
        }
    }

    pub fn from_profiles(profiles: impl IntoIterator<Item = InteriorProfile>) -> Self {
        let mut catalog = Self::new();
        for profile in profiles {
            catalog.insert(profile);
        }
        catalog
    }

    pub fn insert(&mut self, profile: InteriorProfile) {
        self.profiles
            .insert(profile.id.as_str().to_string(), profile);
    }

    pub fn get(&self, id: &InteriorProfileId) -> Option<&InteriorProfile> {
        self.profiles.get(id.as_str())
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }
}

#[cfg(any(test, feature = "dev"))]
pub fn starter_interior_profiles() -> Vec<InteriorProfile> {
    vec![super::profile::two_story_hut_interior_profile()]
}

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_interior_profiles() -> Vec<InteriorProfile> {
    Vec::new()
}

impl Default for InteriorProfileCatalog {
    fn default() -> Self {
        Self::from_profiles(starter_interior_profiles())
    }
}
