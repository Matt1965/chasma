//! Simulation execution control state (ADR-046, ADR-064).

use bevy::prelude::*;

/// Fixed simulation tick duration (30 Hz). Combat timing uses this, not render delta.
pub const SIMULATION_TICK_SECONDS: f32 = 1.0 / 30.0;

/// Maximum authoritative ticks executed per render frame (catch-up cap).
pub const MAX_SIMULATION_TICKS_PER_FRAME: u32 = 8;

/// Global simulation tick gate — independent of rendering, UI, and dev mode.
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct SimulationControlState {
    pub paused: bool,
    pub step_once: bool,
    pub current_tick: u64,
}

impl Default for SimulationControlState {
    fn default() -> Self {
        Self {
            paused: false,
            step_once: false,
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

/// Real-time accumulator that schedules fixed simulation ticks (ADR-064).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct SimulationClock {
    /// Unspent real time carried into the next render frame (seconds).
    pub accumulator_seconds: f32,
}

impl Default for SimulationClock {
    fn default() -> Self {
        Self {
            accumulator_seconds: 0.0,
        }
    }
}

/// Outcome of planning simulation work for one render frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTickPlan {
    pub tick_count: u32,
    /// True when elapsed time still warrants more ticks after the per-frame cap.
    pub capped: bool,
}

impl SimulationClock {
    /// Plan how many fixed simulation ticks should run this render frame.
    ///
    /// Paused simulation does not accumulate real time. `step_once` schedules exactly one
    /// tick without consuming accumulated time.
    pub fn plan_frame(
        &mut self,
        delta_seconds: f32,
        control: &SimulationControlState,
    ) -> FrameTickPlan {
        if control.step_once {
            return FrameTickPlan {
                tick_count: 1,
                capped: false,
            };
        }

        if control.paused {
            return FrameTickPlan {
                tick_count: 0,
                capped: false,
            };
        }

        if delta_seconds > 0.0 {
            self.accumulator_seconds += delta_seconds;
        }

        let mut tick_count = 0u32;
        while self.accumulator_seconds + f32::EPSILON >= SIMULATION_TICK_SECONDS
            && tick_count < MAX_SIMULATION_TICKS_PER_FRAME
        {
            self.accumulator_seconds -= SIMULATION_TICK_SECONDS;
            tick_count += 1;
        }

        let capped = self.accumulator_seconds + f32::EPSILON >= SIMULATION_TICK_SECONDS;
        FrameTickPlan {
            tick_count,
            capped,
        }
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

    fn run_planned_ticks(
        clock: &mut SimulationClock,
        control: &mut SimulationControlState,
        delta_seconds: f32,
    ) -> FrameTickPlan {
        let plan = clock.plan_frame(delta_seconds, control);
        for _ in 0..plan.tick_count {
            assert!(control.begin_tick());
            control.complete_tick();
        }
        plan
    }

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

    #[test]
    fn paused_clock_advances_zero_ticks() {
        let mut clock = SimulationClock::default();
        let mut control = SimulationControlState {
            paused: true,
            ..Default::default()
        };
        let plan = run_planned_ticks(&mut clock, &mut control, 1.0 / 60.0);
        assert_eq!(plan.tick_count, 0);
        assert_eq!(control.current_tick, 0);
        assert_eq!(clock.accumulator_seconds, 0.0);
    }

    #[test]
    fn step_once_clock_advances_exactly_one_tick() {
        let mut clock = SimulationClock::default();
        let mut control = SimulationControlState {
            paused: true,
            step_once: true,
            ..Default::default()
        };
        let plan = run_planned_ticks(&mut clock, &mut control, 1.0 / 60.0);
        assert_eq!(plan.tick_count, 1);
        assert_eq!(control.current_tick, 1);
        assert!(control.paused);
        assert_eq!(clock.accumulator_seconds, 0.0);
    }

    #[test]
    fn thirty_and_sixty_fps_produce_same_tick_count_over_one_second() {
        let mut clock_30 = SimulationClock::default();
        let mut control_30 = SimulationControlState::default();
        for _ in 0..30 {
            run_planned_ticks(&mut clock_30, &mut control_30, 1.0 / 30.0);
        }

        let mut clock_60 = SimulationClock::default();
        let mut control_60 = SimulationControlState::default();
        for _ in 0..60 {
            run_planned_ticks(&mut clock_60, &mut control_60, 1.0 / 60.0);
        }

        assert_eq!(control_30.current_tick, 30);
        assert_eq!(control_60.current_tick, 30);
    }

    #[test]
    fn thirty_and_one_twenty_fps_produce_same_tick_count_over_one_second() {
        let mut clock_30 = SimulationClock::default();
        let mut control_30 = SimulationControlState::default();
        for _ in 0..30 {
            run_planned_ticks(&mut clock_30, &mut control_30, 1.0 / 30.0);
        }

        let mut clock_120 = SimulationClock::default();
        let mut control_120 = SimulationControlState::default();
        for _ in 0..120 {
            run_planned_ticks(&mut clock_120, &mut control_120, 1.0 / 120.0);
        }

        assert_eq!(control_30.current_tick, 30);
        assert_eq!(control_120.current_tick, 30);
    }

    #[test]
    fn slow_render_frame_runs_multiple_ticks_up_to_cap() {
        let mut clock = SimulationClock::default();
        let mut control = SimulationControlState::default();
        let plan = run_planned_ticks(&mut clock, &mut control, 0.5);
        assert_eq!(plan.tick_count, MAX_SIMULATION_TICKS_PER_FRAME);
        assert!(plan.capped);
        assert!(clock.accumulator_seconds > 0.0);
    }

    #[test]
    fn catch_up_continues_across_frames_without_dropping_time() {
        let mut clock = SimulationClock::default();
        let mut control = SimulationControlState::default();
        let first = run_planned_ticks(&mut clock, &mut control, 1.0);
        assert_eq!(first.tick_count, MAX_SIMULATION_TICKS_PER_FRAME);
        assert!(first.capped);

        for _ in 0..10 {
            if control.current_tick >= 30 {
                break;
            }
            run_planned_ticks(&mut clock, &mut control, 0.0);
        }
        if control.current_tick < 30 {
            run_planned_ticks(&mut clock, &mut control, SIMULATION_TICK_SECONDS);
        }
        assert_eq!(control.current_tick, 30);
        assert!(clock.accumulator_seconds < SIMULATION_TICK_SECONDS);
    }

    #[test]
    fn paused_simulation_does_not_accumulate_time() {
        let mut clock = SimulationClock::default();
        let mut control = SimulationControlState {
            paused: true,
            ..Default::default()
        };
        run_planned_ticks(&mut clock, &mut control, 1.0);
        assert_eq!(clock.accumulator_seconds, 0.0);
        control.resume();
        let first = run_planned_ticks(&mut clock, &mut control, 1.0 / 60.0);
        assert_eq!(first.tick_count, 0);
        let second = run_planned_ticks(&mut clock, &mut control, 1.0 / 60.0);
        assert_eq!(second.tick_count, 1);
        assert_eq!(control.current_tick, 1);
    }
}
