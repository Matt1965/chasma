//! Transient SettlementIntent — rebuilt every arbitration, never persisted (SA4).

use bevy::prelude::*;
use std::fmt;

use crate::world::settlement::needs::NeedId;
use crate::world::settlement::response::{ResponseId, ResponseType};
use crate::world::settlement::SettlementId;

/// Stable-within-plan intent identifier (not persisted).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct IntentId(pub String);

impl IntentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn compose(
        settlement_id: SettlementId,
        response_id: &ResponseId,
        need_id: &NeedId,
        tick: u64,
        index: u32,
    ) -> Self {
        Self(format!(
            "{}:{}:{}:{}:{}",
            settlement_id.raw(),
            response_id.as_str(),
            need_id.as_str(),
            tick,
            index
        ))
    }
}

impl fmt::Display for IntentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How long future execution layers should treat this intent as sticky (metadata only in SA4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum IntentPersistence {
    #[default]
    Ephemeral,
    UntilPressureLow,
    Sticky,
}

impl IntentPersistence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ephemeral => "ephemeral",
            Self::UntilPressureLow => "until_pressure_low",
            Self::Sticky => "sticky",
        }
    }
}

/// One chosen strategic response the settlement wishes to pursue.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SettlementIntent {
    pub intent_id: IntentId,
    pub source_need: NeedId,
    pub chosen_response: ResponseId,
    pub response_type: ResponseType,
    /// Arbitration priority (higher = more important). Distinct from SA3 candidate score.
    pub priority: f32,
    pub desired_persistence: IntentPersistence,
    /// Human-readable why this was selected.
    pub reasoning: String,
    pub diagnostics: Vec<String>,
    /// Future AI / faction seam (opaque key/value).
    pub ai_seams: Vec<(String, String)>,
}

/// Why a candidate was not selected into intent.
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum IntentRejectionReason {
    Unavailable,
    ZeroPressure,
    BelowScoreThreshold,
    NeedSlotFull,
    GlobalBudgetFull,
    ConflictWithSelected(String),
    UnknownResponse,
    InvalidScore,
}

impl IntentRejectionReason {
    pub fn label(&self) -> String {
        match self {
            Self::Unavailable => "unavailable".into(),
            Self::ZeroPressure => "zero pressure".into(),
            Self::BelowScoreThreshold => "below score threshold".into(),
            Self::NeedSlotFull => "need selection slots full".into(),
            Self::GlobalBudgetFull => "global intent budget full".into(),
            Self::ConflictWithSelected(other) => format!("conflicts with `{other}`"),
            Self::UnknownResponse => "unknown response".into(),
            Self::InvalidScore => "invalid score".into(),
        }
    }
}

/// Candidate considered but not chosen.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct RejectedIntentCandidate {
    pub response_id: ResponseId,
    pub need_id: NeedId,
    pub candidate_score: f32,
    pub arbitration_score: f32,
    pub reason: IntentRejectionReason,
}

/// Per-settlement arbitration result (chosen + rejected + diagnostics).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SettlementIntentPlan {
    pub settlement_id: SettlementId,
    pub planned_tick: u64,
    /// Tick of the response-candidate set this plan was built from.
    pub source_response_tick: u64,
    /// Tick of the need evaluation used for pressure lookups.
    pub source_need_tick: u64,
    /// Chosen intents, ordered by priority descending.
    pub intents: Vec<SettlementIntent>,
    pub rejected: Vec<RejectedIntentCandidate>,
    pub diagnostics: Vec<String>,
}

impl Default for SettlementIntentPlan {
    fn default() -> Self {
        Self {
            settlement_id: SettlementId::new(0),
            planned_tick: 0,
            source_response_tick: 0,
            source_need_tick: 0,
            intents: Vec::new(),
            rejected: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

impl SettlementIntentPlan {
    pub fn intents_for_need(&self, need_id: &str) -> impl Iterator<Item = &SettlementIntent> + '_ {
        let need_id = need_id.to_owned();
        self.intents
            .iter()
            .filter(move |i| i.source_need.as_str() == need_id)
    }
}
