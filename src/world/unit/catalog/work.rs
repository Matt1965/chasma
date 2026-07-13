use bevy::prelude::*;

/// Per-unit work capability from catalog (ADR-085 B8).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct UnitWorkCapabilities {
    pub can_construct: bool,
    /// Labor units per second toward construction.
    pub construction_speed: f32,
    pub can_operate_workstation: bool,
}

impl Default for UnitWorkCapabilities {
    fn default() -> Self {
        Self {
            can_construct: false,
            construction_speed: 1.0,
            can_operate_workstation: false,
        }
    }
}

impl UnitWorkCapabilities {
    pub fn builder(speed: f32) -> Self {
        Self {
            can_construct: true,
            construction_speed: speed,
            can_operate_workstation: true,
        }
    }
}
