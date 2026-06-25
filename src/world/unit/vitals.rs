use bevy::prelude::*;

/// Authoritative HP state for a unit instance (ADR-055 C2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct UnitVitals {
    pub current_hp: u32,
    pub max_hp: u32,
}

impl UnitVitals {
    pub fn full(max_hp: u32) -> Self {
        Self {
            current_hp: max_hp,
            max_hp,
        }
    }

    pub fn clamped(current_hp: u32, max_hp: u32) -> Self {
        Self {
            current_hp: current_hp.min(max_hp),
            max_hp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_starts_at_max_hp() {
        let vitals = UnitVitals::full(42);
        assert_eq!(vitals.current_hp, 42);
        assert_eq!(vitals.max_hp, 42);
    }

    #[test]
    fn clamped_limits_current_hp() {
        let vitals = UnitVitals::clamped(99, 10);
        assert_eq!(vitals.current_hp, 10);
        assert_eq!(vitals.max_hp, 10);
    }
}
