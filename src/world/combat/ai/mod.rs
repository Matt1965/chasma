//! Basic combat auto-acquisition (ADR-062 C9). Not full tactical AI.

mod acquisition;
mod settings;

pub use acquisition::{
    find_auto_acquire_target, step_combat_ai_acquisition, unit_eligible_for_auto_acquire,
    unit_needs_auto_acquire_target, CombatAiReport, CombatAiScanState, CombatAiTrace,
    CombatAiTraceOutcome,
};
pub use settings::CombatAiSettings;
