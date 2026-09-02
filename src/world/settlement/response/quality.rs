//! SA3 response-quality score components (Phase 6).

use bevy::prelude::*;

use super::definition::ResponseDefinition;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::state::SettlementState;

/// Scale relief (0..1) into a score band comparable with SA4 urgency components.
pub const RELIEF_SCORE_SCALE: f32 = 50.0;

/// Breakdown of SA3 response quality. `total` is the authoritative score.
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Default)]
pub struct ResponseQualityScore {
    pub relief_component: f32,
    pub cost_penalty: f32,
    pub priority_modifier: f32,
    pub emergency_bonus: f32,
    pub total: f32,
}

impl ResponseQualityScore {
    pub fn format_explanation(&self) -> String {
        format!(
            "quality={:.1} (relief={:.1} cost=-{:.1} modifier={:+.1} emergency={:+.1})",
            self.total,
            self.relief_component,
            self.cost_penalty,
            self.priority_modifier,
            self.emergency_bonus,
        )
    }
}

/// Compute response-intrinsic quality. Does not read need pressure, weight, or policy.
pub fn compute_response_quality(
    definition: &ResponseDefinition,
    state: &SettlementState,
    emergency_catalog: &EmergencyCatalog,
    available: bool,
) -> ResponseQualityScore {
    if !available {
        return ResponseQualityScore::default();
    }

    let relief = definition.expected_effect.pressure_relief.clamp(0.0, 1.0);
    let cost = definition.expected_effect.estimated_cost.max(0.0);
    let modifier = f32::from(definition.priority_modifier);
    let emergency_bonus = crate::world::settlement::emergency::emergency_response_score_delta(
        state,
        emergency_catalog,
        definition,
    );

    let relief_component = relief * RELIEF_SCORE_SCALE;
    let total = (relief_component + modifier + emergency_bonus - cost).max(0.0);

    ResponseQualityScore {
        relief_component,
        cost_penalty: cost,
        priority_modifier: modifier,
        emergency_bonus,
        total,
    }
}
