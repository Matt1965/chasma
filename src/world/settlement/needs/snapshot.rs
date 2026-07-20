//! Transient NeedSnapshot — rebuilt every evaluation, never persisted (SA2).

use bevy::prelude::*;

use super::id::NeedId;
use crate::world::settlement::SettlementId;

/// Optional blocking reason when a need cannot be fully measured.
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum NeedBlockingReason {
    MissingSettlementState,
    MissingTarget,
    UnknownEvaluator,
    DataUnavailable(String),
}

impl NeedBlockingReason {
    pub fn label(&self) -> String {
        match self {
            Self::MissingSettlementState => "missing settlement state".into(),
            Self::MissingTarget => "missing target".into(),
            Self::UnknownEvaluator => "unknown evaluator".into(),
            Self::DataUnavailable(detail) => format!("data unavailable: {detail}"),
        }
    }
}

/// Future trend seam — not computed in SA2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum NeedTrend {
    #[default]
    Unknown,
    Rising,
    Stable,
    Falling,
}

/// One need's computed state for one settlement at one tick.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct NeedSnapshot {
    pub need_id: NeedId,
    pub current_value: f32,
    pub desired_value: f32,
    pub deficit: f32,
    pub surplus: f32,
    /// Normalized pressure in 0..=100. Future systems consume this, not raw inventory.
    pub pressure: u8,
    pub blocking_reason: Option<NeedBlockingReason>,
    pub trend: NeedTrend,
    pub evaluated_tick: u64,
    /// Human-readable measurement source for Dev Mode.
    pub evaluation_source: String,
}

impl NeedSnapshot {
    pub fn with_values(
        need_id: NeedId,
        current_value: f32,
        desired_value: f32,
        pressure: u8,
        evaluated_tick: u64,
        evaluation_source: impl Into<String>,
    ) -> Self {
        let deficit = (desired_value - current_value).max(0.0);
        let surplus = (current_value - desired_value).max(0.0);
        Self {
            need_id,
            current_value,
            desired_value,
            deficit,
            surplus,
            pressure,
            blocking_reason: None,
            trend: NeedTrend::Unknown,
            evaluated_tick,
            evaluation_source: evaluation_source.into(),
        }
    }
}

/// Per-settlement transient need evaluation result.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SettlementNeedEvaluation {
    pub settlement_id: SettlementId,
    pub evaluated_tick: u64,
    pub snapshots: Vec<NeedSnapshot>,
    pub diagnostics: Vec<String>,
}

impl Default for SettlementNeedEvaluation {
    fn default() -> Self {
        Self {
            settlement_id: SettlementId::new(0),
            evaluated_tick: 0,
            snapshots: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

impl SettlementNeedEvaluation {
    pub fn snapshot(&self, need_id: &NeedId) -> Option<&NeedSnapshot> {
        self.snapshots.iter().find(|s| &s.need_id == need_id)
    }

    pub fn snapshot_str(&self, need_id: &str) -> Option<&NeedSnapshot> {
        self.snapshots.iter().find(|s| s.need_id.as_str() == need_id)
    }

    pub fn pressure_str(&self, need_id: &str) -> Option<u8> {
        self.snapshot_str(need_id).map(|s| s.pressure)
    }
}
