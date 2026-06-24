//! Simulation execution control state (ADR-046).

use bevy::prelude::*;

/// Global simulation tick gate — independent of rendering, UI, and dev mode.
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct SimulationControlState {
    pub paused: bool,
    pub step_once: bool,
    pub simulation_speed_multiplier: f32,
    pub current_tick: u64,
}

impl Default for SimulationControlState {
    fn default() -> Self {
        Self {
            paused: false,
            step_once: false,
            simulation_speed_multiplier: 1.0,
            current_tick: 0,
        }
    }
}

impl SimulationControlState {
    /// Whether a simulation tick may run this frame.
    pub fn should_advance(&self) -> bool {
        !self.paused || self.step_once
    }

    /// Prepare for a simulation tick. Returns false when paused with no step request.
    pub fn begin_tick(&mut self) -> bool {
        if !self.should_advance() {
            return false;
        }
        true
    }

    /// Record a completed simulation tick and apply step-once pause semantics.
    pub fn complete_tick(&mut self) {
        self.current_tick = self.current_tick.saturating_add(1);
        if self.step_once {
            self.step_once = false;
            self.paused = true;
        }
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if !self.paused {
            self.step_once = false;
        }
    }

    pub fn request_step_once(&mut self) {
        self.step_once = true;
    }

    pub fn pause(&mut self) {
        self.paused = true;
        self.step_once = false;
    }

    pub fn resume(&mut self) {
        self.paused = false;
        self.step_once = false;
    }
}

/// Client-local requests to the simulation controller (dev UI, scripts, etc.).
///
/// Producers set flags; [`apply_simulation_control_requests`] consumes them.
/// Dev mode does not own [`SimulationControlState`].
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct SimulationControlRequests {
    pub toggle_pause: bool,
    pub step_once: bool,
    pub pause: bool,
    pub resume: bool,
}

/// Apply pending control requests from external layers (e.g. dev panel buttons).
pub fn apply_simulation_control_requests(
    mut requests: ResMut<SimulationControlRequests>,
    mut control: ResMut<SimulationControlState>,
) {
    if requests.toggle_pause {
        control.toggle_pause();
        requests.toggle_pause = false;
    }
    if requests.step_once {
        control.request_step_once();
        requests.step_once = false;
    }
    if requests.pause {
        control.pause();
        requests.pause = false;
    }
    if requests.resume {
        control.resume();
        requests.resume = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_prevents_simulation_tick_execution() {
        let mut control = SimulationControlState {
            paused: true,
            ..Default::default()
        };
        assert!(!control.should_advance());
        assert!(!control.begin_tick());
    }

    #[test]
    fn resume_restores_normal_ticking() {
        let mut control = SimulationControlState {
            paused: true,
            ..Default::default()
        };
        control.resume();
        assert!(control.should_advance());
        assert!(control.begin_tick());
    }

    #[test]
    fn step_once_executes_exactly_one_tick() {
        let mut control = SimulationControlState {
            paused: true,
            step_once: true,
            ..Default::default()
        };
        assert!(control.begin_tick());
        control.complete_tick();
        assert_eq!(control.current_tick, 1);
        assert!(!control.step_once);
        assert!(control.paused);
        assert!(!control.should_advance());
    }

    #[test]
    fn step_once_returns_to_paused_state() {
        let mut control = SimulationControlState::default();
        control.pause();
        control.request_step_once();
        assert!(control.begin_tick());
        control.complete_tick();
        assert!(control.paused);
        assert!(!control.step_once);
    }

    #[test]
    fn deterministic_tick_count_behavior() {
        let mut control = SimulationControlState::default();
        for _ in 0..5 {
            assert!(control.begin_tick());
            control.complete_tick();
        }
        assert_eq!(control.current_tick, 5);
    }

    #[test]
    fn toggle_pause_flips_running_state() {
        let mut control = SimulationControlState::default();
        control.toggle_pause();
        assert!(control.paused);
        control.toggle_pause();
        assert!(!control.paused);
    }

    #[test]
    fn resume_clears_pending_step_once() {
        let mut control = SimulationControlState {
            paused: true,
            step_once: true,
            ..Default::default()
        };
        control.resume();
        assert!(!control.step_once);
        assert!(!control.paused);
    }
}
