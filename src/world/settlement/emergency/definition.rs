//! Authored EmergencyDefinition (SA8 / ADR-123).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::settlement::needs::NeedId;
use crate::world::settlement::response::ResponseId;
use crate::world::settlement::state::NeedCategory;
use crate::world::task::TaskPriority;

/// Stable emergency identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct EmergencyId(pub String);

impl EmergencyId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Authored detection evaluator reference (no emergency-name branches in runtime).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyEvaluatorKind {
    /// Signal = clamp(1 - food_stock/desired, 0..=1).
    FoodReserveRatio,
    /// Signal from `extension_seams["hostile_threat"]` (0..=1) or legacy under_attack.
    HostilePresenceSignal,
    /// Signal from `extension_seams["fire_severity"]` (0..=1).
    FireSignal,
    /// Signal from `extension_seams["evacuate_signal"]` or max(hostile, fire).
    EvacuationSignal,
}

/// How need pressure is adjusted while active (scaled by severity unless noted).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NeedPressureModifier {
    pub need_id: NeedId,
    /// Optional category match when need_id empty / wildcard not used — prefer need_id.
    #[serde(default)]
    pub category: Option<NeedCategory>,
    /// Additive pressure bump at severity=1.0 (0..=100 scale).
    pub pressure_delta_at_full: f32,
}

/// Response score adjustment (authored; applied once at SA3).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ResponseScoreModifier {
    /// Exact response id, or empty to match `response_tag`.
    #[serde(default)]
    pub response_id: Option<ResponseId>,
    #[serde(default)]
    pub response_tag: Option<String>,
    /// Score delta at severity=1.0.
    pub score_delta_at_full: f32,
}

/// Interruption policy while this emergency is active.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct EmergencyInterruptionPolicy {
    pub allow_interruption: bool,
    /// Override marketplace stick ticks when interrupting (None = keep default).
    #[serde(default)]
    pub min_stick_ticks: Option<u64>,
    /// Override min priority-rank gap (None = keep default).
    #[serde(default)]
    pub min_priority_rank_gap: Option<u8>,
    /// Tasks at or below this priority are interruptible targets.
    #[serde(default)]
    pub max_interruptible_priority: Option<TaskPriority>,
}

impl Default for EmergencyInterruptionPolicy {
    fn default() -> Self {
        Self {
            allow_interruption: false,
            min_stick_ticks: None,
            min_priority_rank_gap: None,
            max_interruptible_priority: Some(TaskPriority::Normal),
        }
    }
}

/// Task priority bump at strategic emit (one tier at most; not stacked with pressure again).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct TaskPriorityModifier {
    #[serde(default)]
    pub response_tag: Option<String>,
    /// When true, bump Low→Normal or Normal→High for matching strategic emits.
    pub bump_one_tier: bool,
}

/// Authored emergency definition.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct EmergencyDefinition {
    pub id: EmergencyId,
    pub display_name: String,
    pub description: String,
    pub evaluator: EmergencyEvaluatorKind,
    /// Activate when signal >= this (0..=1).
    pub activation_threshold: f32,
    /// Deactivate when signal <= this (must be < activation).
    pub deactivation_threshold: f32,
    pub min_active_duration_ticks: u64,
    #[serde(default)]
    pub recovery_delay_ticks: u64,
    pub need_pressure_modifiers: Vec<NeedPressureModifier>,
    pub response_score_modifiers: Vec<ResponseScoreModifier>,
    /// Response ids unlocked while active (e.g. emergency-only defend).
    #[serde(default)]
    pub unlock_response_ids: Vec<ResponseId>,
    /// Response ids blocked while active.
    #[serde(default)]
    pub block_response_ids: Vec<ResponseId>,
    /// Response tags blocked while active (e.g. "luxury").
    #[serde(default)]
    pub block_response_tags: Vec<String>,
    pub interruption: EmergencyInterruptionPolicy,
    #[serde(default)]
    pub task_priority_modifiers: Vec<TaskPriorityModifier>,
    pub enabled: bool,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

impl EmergencyDefinition {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        evaluator: EmergencyEvaluatorKind,
        activation_threshold: f32,
        deactivation_threshold: f32,
    ) -> Self {
        Self {
            id: EmergencyId::new(id),
            display_name: display_name.into(),
            description: description.into(),
            evaluator,
            activation_threshold,
            deactivation_threshold,
            min_active_duration_ticks: 60,
            recovery_delay_ticks: 0,
            need_pressure_modifiers: Vec::new(),
            response_score_modifiers: Vec::new(),
            unlock_response_ids: Vec::new(),
            block_response_ids: Vec::new(),
            block_response_tags: Vec::new(),
            interruption: EmergencyInterruptionPolicy::default(),
            task_priority_modifiers: Vec::new(),
            enabled: true,
            diagnostics: Vec::new(),
        }
    }
}
