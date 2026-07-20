//! Transient CandidateResponse — rebuilt every discovery, never persisted (SA3).

use bevy::prelude::*;

use super::definition::ResponseType;
use super::id::ResponseId;
use crate::world::settlement::needs::NeedId;
use crate::world::settlement::SettlementId;
use crate::world::BuildingId;

/// Whether a candidate can be considered for future selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ResponseAvailability {
    Available,
    Unavailable,
}

impl ResponseAvailability {
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Why a candidate is unavailable (or partially blocked).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum ResponseBlockingReason {
    MissingCapability(String),
    PolicyDisabled(String),
    PrerequisiteUnmet(String),
    ZeroPressure,
    DefinitionDisabled,
    /// Blocked or not unlocked by an active emergency (SA8).
    Emergency(String),
}

impl ResponseBlockingReason {
    pub fn label(&self) -> String {
        match self {
            Self::MissingCapability(detail) => format!("missing capability: {detail}"),
            Self::PolicyDisabled(detail) => format!("policy disabled: {detail}"),
            Self::PrerequisiteUnmet(detail) => format!("prerequisite unmet: {detail}"),
            Self::ZeroPressure => "need pressure is zero".into(),
            Self::DefinitionDisabled => "response definition disabled".into(),
            Self::Emergency(detail) => format!("emergency: {detail}"),
        }
    }
}

/// One scored response option for one need at one settlement.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct CandidateResponse {
    pub response_id: ResponseId,
    pub need_id: NeedId,
    pub response_type: ResponseType,
    /// Expected pressure relief copied from definition (0..=1).
    pub expected_impact: f32,
    /// Estimated abstract cost from definition.
    pub estimated_cost: f32,
    pub availability: ResponseAvailability,
    pub blocking_reason: Option<ResponseBlockingReason>,
    /// Deterministic priority score (higher = more attractive). Unavailable → 0.
    pub priority_score: f32,
    pub supporting_buildings: Vec<BuildingId>,
    pub diagnostics: Vec<String>,
}

impl CandidateResponse {
    pub fn is_available(&self) -> bool {
        self.availability.is_available()
    }
}

/// Per-settlement transient response discovery result.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SettlementResponseCandidates {
    pub settlement_id: SettlementId,
    pub evaluated_tick: u64,
    /// Tick of the NeedEvaluation this discovery was built from.
    pub source_need_tick: u64,
    pub candidates: Vec<CandidateResponse>,
    pub diagnostics: Vec<String>,
}

impl Default for SettlementResponseCandidates {
    fn default() -> Self {
        Self {
            settlement_id: SettlementId::new(0),
            evaluated_tick: 0,
            source_need_tick: 0,
            candidates: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

impl SettlementResponseCandidates {
    pub fn available(&self) -> impl Iterator<Item = &CandidateResponse> {
        self.candidates.iter().filter(|c| c.is_available())
    }

    pub fn for_need<'a>(
        &'a self,
        need_id: &str,
    ) -> impl Iterator<Item = &'a CandidateResponse> + 'a {
        let need_id = need_id.to_owned();
        self.candidates
            .iter()
            .filter(move |c| c.need_id.as_str() == need_id)
    }

    pub fn best_for_need(&self, need_id: &str) -> Option<&CandidateResponse> {
        self.for_need(need_id)
            .filter(|c| c.is_available())
            .max_by(|a, b| {
                a.priority_score
                    .partial_cmp(&b.priority_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}
