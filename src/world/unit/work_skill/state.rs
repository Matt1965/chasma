//! Per-unit work skill progression state.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::catalog::WorkSkillCatalog;
use super::id::WorkSkillId;

/// Baseline skill value when no explicit override exists. Not a maximum scale.
pub const DEFAULT_WORK_SKILL_VALUE: i64 = 0;

/// Mutable per-unit work skill overrides. Absent keys resolve via [`DEFAULT_WORK_SKILL_VALUE`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub struct UnitWorkSkillState {
    #[serde(default)]
    overrides: BTreeMap<WorkSkillId, i64>,
}

impl UnitWorkSkillState {
    pub fn resolve(&self, skill_id: &WorkSkillId) -> i64 {
        self.overrides
            .get(skill_id)
            .copied()
            .unwrap_or(DEFAULT_WORK_SKILL_VALUE)
    }

    pub fn set(&mut self, skill_id: WorkSkillId, value: i64) {
        self.overrides.insert(skill_id, value);
    }

    pub fn overrides(&self) -> &BTreeMap<WorkSkillId, i64> {
        &self.overrides
    }

    pub fn overrides_mut(&mut self) -> &mut BTreeMap<WorkSkillId, i64> {
        &mut self.overrides
    }

    /// Resolve every currently-authored enabled skill for presentation and future workforce UI.
    pub fn resolved_values(&self, catalog: &WorkSkillCatalog) -> Vec<(WorkSkillId, i64)> {
        catalog
            .enabled_definitions_ordered()
            .iter()
            .map(|definition| (definition.id.clone(), self.resolve(&definition.id)))
            .collect()
    }
}
