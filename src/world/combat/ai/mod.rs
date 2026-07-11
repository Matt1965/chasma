//! Basic combat auto-acquisition (ADR-062 C9). Not full tactical AI.

mod acquisition;
mod settings;

pub use acquisition::{
    CombatAiReport, CombatAiScanState, CombatAiTrace, CombatAiTraceOutcome,
    find_auto_acquire_target, step_combat_ai_acquisition,
};
pub use settings::CombatAiSettings;
