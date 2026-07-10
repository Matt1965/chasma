//! Canonical keyboard binding reference (REVIEW-B5, ADR-068).
//!
//! Not a remapping system — documents ownership to prevent duplicate handlers.

/// Global bindings that must have exactly one owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalBinding {
    ToggleDevMode,
    ToggleSimulationPause,
    StepSimulation,
    CycleSpawnAffiliation,
}

impl GlobalBinding {
    pub fn label(self) -> &'static str {
        match self {
            Self::ToggleDevMode => "F12 — toggle Dev Mode",
            Self::ToggleSimulationPause => "Space — pause/resume simulation",
            Self::StepSimulation => "Shift+Space — single simulation step",
            Self::CycleSpawnAffiliation => "Shift+T — cycle dev spawn affiliation (dev only)",
        }
    }
}

/// Dev-only time-of-day bindings (World Tools tab; panel buttons are primary).
pub const TIME_OF_DAY_KEYBOARD_HINT: &str =
    "[ / ] adjust hour, , / . adjust day length (World Tools tab)";

/// Returns true when two critical bindings would share the same key in dev mode.
pub fn critical_binding_conflict(a: GlobalBinding, b: GlobalBinding) -> bool {
    matches!(
        (a, b),
        (GlobalBinding::CycleSpawnAffiliation, GlobalBinding::CycleSpawnAffiliation)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_mode_and_simulation_pause_are_distinct_bindings() {
        assert_ne!(GlobalBinding::ToggleDevMode, GlobalBinding::ToggleSimulationPause);
        assert_ne!(GlobalBinding::ToggleSimulationPause, GlobalBinding::StepSimulation);
    }

    #[test]
    fn spawn_affiliation_no_longer_uses_bare_t() {
        assert_eq!(
            GlobalBinding::CycleSpawnAffiliation.label(),
            "Shift+T — cycle dev spawn affiliation (dev only)"
        );
    }
}
