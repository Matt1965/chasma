//! Source profile catalog (ADR-102).

use std::collections::BTreeMap;
use std::path::Path;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::id::{TerrainFieldId, TerrainFieldSourceProfileId};
use super::super::source_error::TerrainFieldSourceError;
use super::profile::TerrainFieldSourceProfileDefinition;

use super::starter;

#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
pub struct TerrainFieldSourceProfileCatalogRon {
    pub profiles: Vec<TerrainFieldSourceProfileDefinition>,
}

/// Read-only registry of terrain field source profiles.
#[derive(Debug, Clone, Resource)]
pub struct TerrainFieldSourceProfileCatalog {
    profiles: Vec<TerrainFieldSourceProfileDefinition>,
    by_id: BTreeMap<TerrainFieldSourceProfileId, usize>,
    by_field: BTreeMap<TerrainFieldId, usize>,
}

impl Default for TerrainFieldSourceProfileCatalog {
    fn default() -> Self {
        Self::from_profiles(starter::starter_source_profiles())
            .expect("starter source profiles are valid")
    }
}

impl TerrainFieldSourceProfileCatalog {
    pub fn from_profiles(
        profiles: Vec<TerrainFieldSourceProfileDefinition>,
    ) -> Result<Self, TerrainFieldSourceError> {
        let mut by_id = BTreeMap::new();
        let mut by_field = BTreeMap::new();
        for (index, profile) in profiles.iter().enumerate() {
            profile.validate()?;
            if by_id.insert(profile.id.clone(), index).is_some() {
                return Err(TerrainFieldSourceError::DuplicateTerrainFieldSourceProfile(
                    profile.id.clone(),
                ));
            }
            if by_field
                .insert(profile.output_field_id.clone(), index)
                .is_some()
            {
                return Err(TerrainFieldSourceError::InvalidSourceConfiguration(
                    format!(
                        "duplicate output_field_id `{}` on profile `{}`",
                        profile.output_field_id, profile.id
                    ),
                ));
            }
        }
        Ok(Self {
            profiles,
            by_id,
            by_field,
        })
    }

    pub fn get(
        &self,
        id: &TerrainFieldSourceProfileId,
    ) -> Option<&TerrainFieldSourceProfileDefinition> {
        self.by_id.get(id).map(|i| &self.profiles[*i])
    }

    pub fn for_field(
        &self,
        field_id: &TerrainFieldId,
    ) -> Option<&TerrainFieldSourceProfileDefinition> {
        self.by_field.get(field_id).map(|i| &self.profiles[*i])
    }

    pub fn profiles(&self) -> &[TerrainFieldSourceProfileDefinition] {
        &self.profiles
    }

    pub fn enabled_profiles(&self) -> impl Iterator<Item = &TerrainFieldSourceProfileDefinition> {
        self.profiles.iter().filter(|p| p.enabled)
    }

    pub fn load_from_ron_path(path: &Path) -> Result<Self, TerrainFieldSourceError> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| TerrainFieldSourceError::InvalidSourceConfiguration(err.to_string()))?;
        Self::load_from_ron(&text)
    }

    pub fn load_from_ron(text: &str) -> Result<Self, TerrainFieldSourceError> {
        let file: TerrainFieldSourceProfileCatalogRon = ron::from_str(text)
            .map_err(|err| TerrainFieldSourceError::InvalidSourceConfiguration(err.to_string()))?;
        Self::from_profiles(file.profiles)
    }
}

pub const TERRAIN_FIELD_SOURCE_PROFILES_RON_PATH: &str =
    "assets/terrain_fields/source_profiles.ron";

pub fn load_terrain_field_source_profile_catalog() -> TerrainFieldSourceProfileCatalog {
    TerrainFieldSourceProfileCatalog::load_from_ron_path(Path::new(
        TERRAIN_FIELD_SOURCE_PROFILES_RON_PATH,
    ))
    .unwrap_or_else(|err| {
        panic!(
            "failed to load terrain field source profiles from {TERRAIN_FIELD_SOURCE_PROFILES_RON_PATH}: {err}"
        )
    })
}
