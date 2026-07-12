//! Presentation timer advancement aligned with simulation pause/step (A1).

use std::time::Duration;

use crate::simulation::{SIMULATION_TICK_SECONDS, SimulationControlState};

/// Seconds of presentation time that may advance this frame.
///
/// - **Paused:** `0` — death/hit timers and clip advancement tied to sim freeze.
/// - **Step once:** exactly one simulation tick — not render-frame delta.
/// - **Running:** render-frame delta (visual-only locomotion polish).
pub fn presentation_advance_seconds(
    control: &SimulationControlState,
    render_delta_seconds: f32,
) -> f32 {
    if control.paused && !control.step_once {
        0.0
    } else if control.step_once {
        SIMULATION_TICK_SECONDS
    } else {
        render_delta_seconds.max(0.0)
    }
}

/// Whether presentation timers should advance this frame.
pub fn presentation_timers_advance(control: &SimulationControlState) -> bool {
    presentation_advance_seconds(control, 0.0) > 0.0 || control.step_once
}

/// Default attack blend-out when weapon metadata is absent (A1).
pub fn default_attack_blend_out() -> Duration {
    Duration::from_millis(150)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paused() -> SimulationControlState {
        SimulationControlState {
            paused: true,
            step_once: false,
            current_tick: 0,
        }
    }

    fn stepping() -> SimulationControlState {
        SimulationControlState {
            paused: true,
            step_once: true,
            current_tick: 0,
        }
    }

    #[test]
    fn paused_presentation_delta_is_zero() {
        assert_eq!(presentation_advance_seconds(&paused(), 0.016), 0.0);
        assert!(!presentation_timers_advance(&paused()));
    }

    #[test]
    fn step_once_uses_simulation_tick() {
        assert_eq!(
            presentation_advance_seconds(&stepping(), 0.05),
            SIMULATION_TICK_SECONDS
        );
        assert!(presentation_timers_advance(&stepping()));
    }

    #[test]
    fn running_uses_render_delta() {
        let control = SimulationControlState::default();
        assert_eq!(presentation_advance_seconds(&control, 0.02), 0.02);
    }

    #[test]
    fn resume_continues_from_prior_timer_values() {
        let mut control = paused();
        let mut remaining = 2.0f32;
        remaining -= presentation_advance_seconds(&control, 0.1);
        assert_eq!(remaining, 2.0);
        control.resume();
        remaining -= presentation_advance_seconds(&control, 0.5);
        assert_eq!(remaining, 1.5);
    }
}
