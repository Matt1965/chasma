use bevy::prelude::*;

/// High-level combat posture for a unit instance (ADR-055 C2, ADR-056 C3).
///
/// Locomotion remains on [`super::state::UnitState`]. Attack intent and targets
/// are stored here so movement and combat concerns stay separated.
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
pub enum CombatState {
    #[default]
    Peaceful,
    /// Placeholder for future alert posture.
    Alert,
    /// Placeholder for future engaged posture.
    Engaged,
    /// Attack order assigned — in weapon range, ready (no damage in C4).
    Attacking {
        target: crate::world::UnitId,
    },
    /// Pursuing an attack target to enter weapon range.
    Chasing {
        target: crate::world::UnitId,
    },
    /// Attack-move order assigned — destination + optional acquired target.
    AttackMoving {
        destination: crate::world::WorldPosition,
        target: Option<crate::world::UnitId>,
    },
}
impl CombatState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Peaceful => "Peaceful",
            Self::Alert => "Alert",
            Self::Engaged => "Engaged",
            Self::Attacking { .. } => "Attacking",
            Self::Chasing { .. } => "Chasing",
            Self::AttackMoving { .. } => "AttackMoving",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_peaceful() {
        assert_eq!(CombatState::default(), CombatState::Peaceful);
    }
}
