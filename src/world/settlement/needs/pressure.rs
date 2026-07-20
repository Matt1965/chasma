//! Pressure normalization (SA2). Pressure is the universal need output (0..=100).

use crate::world::settlement::state::SettlementModifier;

/// Convert current/desired into a stable normalized pressure in `0..=100`.
///
/// `pressure = clamp(round((max(0, desired - current) / desired) * 100), 0, 100)`
/// When `desired <= 0`, pressure is 0 (no unmet demand).
pub fn normalize_pressure(current: f32, desired: f32) -> u8 {
    if !current.is_finite() || !desired.is_finite() {
        return 0;
    }
    if desired <= 0.0 {
        return 0;
    }
    let deficit = (desired - current).max(0.0);
    let ratio = deficit / desired;
    (ratio * 100.0).clamp(0.0, 100.0).round() as u8
}

/// Apply SettlementState modifiers whose key matches the need id (or `"all"`).
/// Magnitude is treated as additive pressure points (−100..=100 typical).
pub fn apply_pressure_modifiers(
    base_pressure: u8,
    need_id: &str,
    modifiers: &[SettlementModifier],
    simulation_tick: u64,
) -> u8 {
    let mut pressure = f32::from(base_pressure);
    for modifier in modifiers {
        if let Some(expires) = modifier.expires_tick {
            if simulation_tick >= expires {
                continue;
            }
        }
        if modifier.key == need_id || modifier.key == "all" {
            if modifier.magnitude.is_finite() {
                pressure += modifier.magnitude;
            }
        }
    }
    pressure.clamp(0.0, 100.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::settlement::state::{SettlementModifier, SettlementModifierSource};

    #[test]
    fn pressure_full_deficit_is_100() {
        assert_eq!(normalize_pressure(0.0, 100.0), 100);
    }

    #[test]
    fn pressure_half_deficit_is_50() {
        assert_eq!(normalize_pressure(50.0, 100.0), 50);
    }

    #[test]
    fn pressure_met_is_zero() {
        assert_eq!(normalize_pressure(100.0, 100.0), 0);
        assert_eq!(normalize_pressure(150.0, 100.0), 0);
    }

    #[test]
    fn pressure_zero_desired_is_zero() {
        assert_eq!(normalize_pressure(0.0, 0.0), 0);
    }

    #[test]
    fn modifiers_adjust_pressure() {
        let mods = [SettlementModifier {
            source: SettlementModifierSource::Scenario,
            key: "food".into(),
            magnitude: 25.0,
            expires_tick: None,
        }];
        assert_eq!(apply_pressure_modifiers(50, "food", &mods, 0), 75);
    }
}
