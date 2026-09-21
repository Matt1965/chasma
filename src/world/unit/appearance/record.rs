use std::collections::BTreeMap;

use bevy::prelude::*;

use super::id::{AppearanceParamId, AppearanceProfileId, BodyVariantId};

/// Persisted per-unit visual appearance (CG1).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct UnitAppearance {
    pub profile_id: AppearanceProfileId,
    pub body_variant_id: BodyVariantId,
    pub height_scale: f32,
    /// Semantic parameter values — not technical morph indices.
    pub morphs: BTreeMap<AppearanceParamId, f32>,
    /// Optional reproducibility metadata; not authoritative over resolved values.
    pub generation_seed: Option<u64>,
}

impl UnitAppearance {
    pub fn semantic_value(&self, param_id: &AppearanceParamId) -> Option<f32> {
        self.morphs.get(param_id).copied()
    }
}
