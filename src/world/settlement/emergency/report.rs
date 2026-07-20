//! Transient emergency evaluation report (SA8). Never persisted.

use bevy::prelude::*;

use crate::world::settlement::SettlementId;

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct EmergencySignalDiagnostic {
    pub emergency_id: String,
    pub signal: f32,
    pub activation_threshold: f32,
    pub deactivation_threshold: f32,
    pub evaluator: String,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct EmergencyEvaluationReport {
    pub settlement_id: SettlementId,
    pub evaluated_tick: u64,
    pub signals: Vec<EmergencySignalDiagnostic>,
    pub activated: Vec<String>,
    pub deactivated: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl Default for EmergencyEvaluationReport {
    fn default() -> Self {
        Self {
            settlement_id: SettlementId::new(0),
            evaluated_tick: 0,
            signals: Vec::new(),
            activated: Vec::new(),
            deactivated: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
