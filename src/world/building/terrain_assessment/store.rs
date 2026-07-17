use std::collections::HashMap;

use bevy::prelude::*;

use super::revision::BuildingTerrainAssessmentKey;
use super::types::BuildingTerrainAssessment;
use crate::world::BuildingId;

/// Cached authoritative terrain assessments keyed by building id (ADR-104 TF4).
#[derive(Debug, Clone, Default, Resource, Reflect)]
#[reflect(Resource)]
pub struct BuildingTerrainAssessmentStore {
    assessments: HashMap<BuildingId, BuildingTerrainAssessment>,
    #[reflect(ignore)]
    keys: HashMap<BuildingId, BuildingTerrainAssessmentKey>,
    #[reflect(ignore)]
    dirty: HashMap<BuildingId, bool>,
}

impl BuildingTerrainAssessmentStore {
    pub fn get(&self, building_id: BuildingId) -> Option<&BuildingTerrainAssessment> {
        self.assessments.get(&building_id)
    }

    pub fn insert(
        &mut self,
        building_id: BuildingId,
        key: BuildingTerrainAssessmentKey,
        assessment: BuildingTerrainAssessment,
    ) {
        self.keys.insert(building_id, key);
        self.assessments.insert(building_id, assessment);
        self.dirty.insert(building_id, false);
    }

    pub fn mark_dirty(&mut self, building_id: BuildingId) {
        self.dirty.insert(building_id, true);
        if let Some(assessment) = self.assessments.get_mut(&building_id) {
            assessment.stale = true;
        }
    }

    pub fn mark_all_dirty(&mut self) {
        for id in self.assessments.keys().cloned().collect::<Vec<_>>() {
            self.mark_dirty(id);
        }
    }

    pub fn remove(&mut self, building_id: BuildingId) {
        self.assessments.remove(&building_id);
        self.keys.remove(&building_id);
        self.dirty.remove(&building_id);
    }

    pub fn is_dirty(&self, building_id: BuildingId) -> bool {
        self.dirty.get(&building_id).copied().unwrap_or(true)
    }

    pub fn is_cache_valid(
        &self,
        building_id: BuildingId,
        key: &BuildingTerrainAssessmentKey,
    ) -> bool {
        self.keys.get(&building_id) == Some(key) && !self.is_dirty(building_id)
    }

    pub fn stored_key(&self, building_id: BuildingId) -> Option<&BuildingTerrainAssessmentKey> {
        self.keys.get(&building_id)
    }

    pub fn len(&self) -> usize {
        self.assessments.len()
    }
}
