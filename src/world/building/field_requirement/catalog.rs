//! Read-only registry of building field requirements (ADR-104 TF4).

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::definition::{BuildingFieldRequirementDefinition, BuildingFieldRequirementKind};
use super::error::BuildingFieldRequirementError;
use super::starter;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::field_response::FieldResponseProfileCatalog;
use crate::world::{BuildingDefinitionId, FootprintCatalog, TerrainFieldCatalog, TerrainFieldId};

/// Monotonic revision bumped when requirement catalog content changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect, Resource)]
pub struct BuildingFieldRequirementCatalogRevision(pub u64);

#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct BuildingFieldRequirementCatalog {
    requirements: Vec<BuildingFieldRequirementDefinition>,
    #[reflect(ignore)]
    by_building: HashMap<BuildingDefinitionId, Vec<usize>>,
    #[reflect(ignore)]
    by_building_field: HashMap<(BuildingDefinitionId, TerrainFieldId), usize>,
}

impl Default for BuildingFieldRequirementCatalog {
    fn default() -> Self {
        Self::from_definitions(starter::starter_requirements())
            .expect("starter building field requirements are valid")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingFieldRequirementCatalogRon {
    pub requirements: Vec<BuildingFieldRequirementDefinition>,
}

impl BuildingFieldRequirementCatalog {
    pub fn from_definitions(
        requirements: Vec<BuildingFieldRequirementDefinition>,
    ) -> Result<Self, BuildingFieldRequirementError> {
        let mut sorted = requirements;
        sorted.sort_by_key(|req| req.sort_key());

        let mut by_building: HashMap<BuildingDefinitionId, Vec<usize>> = HashMap::new();
        let mut by_building_field = HashMap::new();
        let mut primary_counts: HashMap<BuildingDefinitionId, usize> = HashMap::new();

        for (index, requirement) in sorted.iter().enumerate() {
            if !requirement.enabled {
                continue;
            }
            if requirement.minimum_usable_coverage_basis_points > 10_000 {
                return Err(BuildingFieldRequirementError::InvalidCoverageRequirement {
                    building_id: requirement.building_definition_id.clone(),
                    field_id: requirement.terrain_field_id.clone(),
                    coverage_basis_points: requirement.minimum_usable_coverage_basis_points,
                });
            }
            let key = (
                requirement.building_definition_id.clone(),
                requirement.terrain_field_id.clone(),
            );
            if by_building_field.insert(key, index).is_some() {
                return Err(BuildingFieldRequirementError::DuplicateRequirement {
                    building_id: requirement.building_definition_id.clone(),
                    field_id: requirement.terrain_field_id.clone(),
                });
            }
            if requirement.primary_overlay {
                let count = primary_counts
                    .entry(requirement.building_definition_id.clone())
                    .or_insert(0);
                *count += 1;
                if *count > 1 {
                    return Err(BuildingFieldRequirementError::PrimaryOverlayConflict(
                        requirement.building_definition_id.clone(),
                    ));
                }
            }
            by_building
                .entry(requirement.building_definition_id.clone())
                .or_default()
                .push(index);
        }

        Ok(Self {
            requirements: sorted,
            by_building,
            by_building_field,
        })
    }

    pub fn requirements_for_building(
        &self,
        building_id: &BuildingDefinitionId,
    ) -> Vec<&BuildingFieldRequirementDefinition> {
        self.by_building
            .get(building_id)
            .map(|indices| {
                indices
                    .iter()
                    .map(|index| &self.requirements[*index])
                    .filter(|req| req.enabled)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn active_required_efficiency(
        &self,
        building_id: &BuildingDefinitionId,
    ) -> Vec<&BuildingFieldRequirementDefinition> {
        self.requirements_for_building(building_id)
            .into_iter()
            .filter(|req| req.requirement_kind == BuildingFieldRequirementKind::RequiredEfficiency)
            .collect()
    }

    pub fn lookup(
        &self,
        building_id: &BuildingDefinitionId,
        field_id: &TerrainFieldId,
    ) -> Option<&BuildingFieldRequirementDefinition> {
        self.by_building_field
            .get(&(building_id.clone(), field_id.clone()))
            .map(|index| &self.requirements[*index])
    }

    pub fn primary_overlay_field(
        &self,
        building_id: &BuildingDefinitionId,
    ) -> Option<TerrainFieldId> {
        let mut reqs = self.active_required_efficiency(building_id);
        if reqs.is_empty() {
            return None;
        }
        if let Some(primary) = reqs.iter().find(|req| req.primary_overlay) {
            return Some(primary.terrain_field_id.clone());
        }
        reqs.sort_by_key(|req| {
            (
                req.overlay_priority,
                req.terrain_field_id.as_str().to_string(),
            )
        });
        Some(reqs[0].terrain_field_id.clone())
    }

    pub fn definitions(&self) -> &[BuildingFieldRequirementDefinition] {
        &self.requirements
    }

    pub fn validate_against_catalogs(
        &self,
        buildings: &BuildingCatalog,
        fields: &TerrainFieldCatalog,
        profiles: &FieldResponseProfileCatalog,
        footprints: &FootprintCatalog,
    ) -> Vec<BuildingFieldRequirementError> {
        let mut errors = Vec::new();
        for requirement in self.requirements.iter().filter(|req| req.enabled) {
            if let Err(err) = super::error::validate_requirement(
                requirement,
                buildings.get(&requirement.building_definition_id).is_some(),
                fields.get(&requirement.terrain_field_id).is_some(),
                fields
                    .get(&requirement.terrain_field_id)
                    .is_some_and(|field| field.enabled),
                profiles.get(&requirement.response_profile_id).is_some(),
                profiles
                    .get(&requirement.response_profile_id)
                    .is_some_and(|profile| profile.enabled),
                requirement
                    .sampling_footprint_id
                    .as_ref()
                    .is_none_or(|id| footprints.get(id).is_some()),
            ) {
                errors.push(err);
            }
        }
        errors
    }

    pub fn load_from_ron_path(path: &Path) -> Result<Self, BuildingFieldRequirementError> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| BuildingFieldRequirementError::RonIo(err.to_string()))?;
        Self::load_from_ron(&text)
    }

    pub fn load_from_ron(text: &str) -> Result<Self, BuildingFieldRequirementError> {
        let file: BuildingFieldRequirementCatalogRon = ron::from_str(text)
            .map_err(|err| BuildingFieldRequirementError::RonParse(err.to_string()))?;
        Self::from_definitions(file.requirements)
    }
}

pub const BUILDING_FIELD_REQUIREMENT_CATALOG_RON_PATH: &str =
    "assets/building_field_requirements/catalog.ron";

pub fn load_building_field_requirement_catalog() -> BuildingFieldRequirementCatalog {
    BuildingFieldRequirementCatalog::load_from_ron_path(Path::new(
        BUILDING_FIELD_REQUIREMENT_CATALOG_RON_PATH,
    ))
    .unwrap_or_else(|err| {
        bevy::log::warn!(
            "building field requirement catalog missing or invalid at {BUILDING_FIELD_REQUIREMENT_CATALOG_RON_PATH} ({err}); using starter requirements"
        );
        BuildingFieldRequirementCatalog::default()
    })
}
