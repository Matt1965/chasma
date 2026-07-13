use bevy::prelude::*;

/// Authoritative HP state for a building instance (ADR-082 B5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct BuildingVitals {
    pub current_hp: u32,
    pub max_hp: u32,
}

impl BuildingVitals {
    pub fn full(max_hp: u32) -> Self {
        let max_hp = max_hp.max(1);
        Self {
            current_hp: max_hp,
            max_hp,
        }
    }

    pub fn construction_vulnerable(max_hp: u32) -> Self {
        let max_hp = max_hp.max(1);
        let current_hp = (max_hp / 10).max(1);
        Self { current_hp, max_hp }
    }

    pub fn clamped(current_hp: u32, max_hp: u32) -> Self {
        let max_hp = max_hp.max(1);
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
        let vitals = BuildingVitals::full(250);
        assert_eq!(vitals.current_hp, 250);
        assert_eq!(vitals.max_hp, 250);
    }

    #[test]
    fn construction_vulnerable_uses_ten_percent() {
        let vitals = BuildingVitals::construction_vulnerable(250);
        assert_eq!(vitals.current_hp, 25);
        assert_eq!(vitals.max_hp, 250);
    }
}
