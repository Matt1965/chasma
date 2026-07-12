use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::AnimationProfile;
use super::id::AnimationProfileId;
use super::starter::starter_definitions;

/// Read-only registry of animation profiles (A1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct AnimationProfileCatalog {
    definitions: Vec<AnimationProfile>,
    by_id: HashMap<AnimationProfileId, usize>,
}

impl Default for AnimationProfileCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_definitions()).expect("animation profile catalog is valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimationProfileCatalogError {
    DuplicateId(AnimationProfileId),
}

impl AnimationProfileCatalog {
    pub fn from_definitions(
        definitions: Vec<AnimationProfile>,
    ) -> Result<Self, AnimationProfileCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(AnimationProfileCatalogError::DuplicateId(
                    definition.id.clone(),
                ));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn get(&self, id: &AnimationProfileId) -> Option<&AnimationProfile> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn definitions(&self) -> &[AnimationProfile] {
        &self.definitions
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}
