use bevy::prelude::*;

/// Per-unit physical work capability from catalog (ADR-085 B8, ADR-115/122).
///
/// These flags express whether a unit type can physically perform a task kind.
/// They are not profession, skill, player permission, or performance.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct UnitWorkCapabilities {
    pub can_construct: bool,
    /// Labor units per second toward construction (performance, not eligibility).
    pub construction_speed: f32,
    pub can_operate_workstation: bool,
    pub can_haul: bool,
}

impl Default for UnitWorkCapabilities {
    fn default() -> Self {
        Self {
            can_construct: false,
            construction_speed: 1.0,
            can_operate_workstation: false,
            can_haul: false,
        }
    }
}

impl UnitWorkCapabilities {
    /// Ordinary settler defaults: construct, operate, and haul.
    pub fn settler_default() -> Self {
        Self {
            can_construct: true,
            construction_speed: 1.0,
            can_operate_workstation: true,
            can_haul: true,
        }
    }

    pub fn builder(speed: f32) -> Self {
        Self {
            can_construct: true,
            construction_speed: speed,
            can_operate_workstation: true,
            can_haul: true,
        }
    }
}
