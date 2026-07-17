//! Transform editing capability policy (ADR-097 DT1).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Per-axis transform editing permissions for Dev Mode (DT2+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub struct TransformCapabilities {
    pub translate_x: bool,
    pub translate_y: bool,
    pub translate_z: bool,
    pub rotate_x: bool,
    pub rotate_y: bool,
    pub rotate_z: bool,
    pub uniform_scale: bool,
    pub nonuniform_scale: bool,
    pub delete: bool,
    pub duplicate: bool,
}

impl TransformCapabilities {
    pub const NONE: Self = Self {
        translate_x: false,
        translate_y: false,
        translate_z: false,
        rotate_x: false,
        rotate_y: false,
        rotate_z: false,
        uniform_scale: false,
        nonuniform_scale: false,
        delete: false,
        duplicate: false,
    };

    pub fn doodad() -> Self {
        Self {
            translate_x: true,
            translate_y: true,
            translate_z: true,
            rotate_x: true,
            rotate_y: true,
            rotate_z: true,
            uniform_scale: true,
            nonuniform_scale: true,
            delete: true,
            duplicate: true,
        }
    }

    pub fn navigable_building() -> Self {
        Self {
            translate_x: true,
            translate_y: true,
            translate_z: true,
            rotate_x: false,
            rotate_y: true,
            rotate_z: false,
            uniform_scale: true,
            nonuniform_scale: false,
            delete: true,
            duplicate: true,
        }
    }

    pub fn decorative_building() -> Self {
        Self {
            translate_x: true,
            translate_y: true,
            translate_z: true,
            rotate_x: true,
            rotate_y: true,
            rotate_z: true,
            uniform_scale: true,
            nonuniform_scale: false,
            delete: true,
            duplicate: true,
        }
    }

    pub fn unit() -> Self {
        Self::NONE
    }

    pub fn world_item_pile() -> Self {
        Self {
            translate_x: true,
            translate_y: true,
            translate_z: true,
            ..Self::NONE
        }
    }

    pub fn corpse() -> Self {
        Self {
            translate_x: true,
            translate_y: true,
            translate_z: true,
            rotate_y: true,
            delete: true,
            ..Self::NONE
        }
    }

    pub fn door() -> Self {
        Self::NONE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum BuildingTransformSafetyClass {
    Navigable,
    DecorativeNonNavigable,
}

impl BuildingTransformSafetyClass {
    pub fn capabilities(self) -> TransformCapabilities {
        match self {
            Self::Navigable => TransformCapabilities::navigable_building(),
            Self::DecorativeNonNavigable => TransformCapabilities::decorative_building(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doodad_has_full_capabilities() {
        let caps = TransformCapabilities::doodad();
        assert!(caps.nonuniform_scale);
        assert!(caps.rotate_x);
    }

    #[test]
    fn unit_editing_disabled() {
        assert_eq!(TransformCapabilities::unit(), TransformCapabilities::NONE);
    }

    #[test]
    fn navigable_building_yaw_only() {
        let caps = TransformCapabilities::navigable_building();
        assert!(caps.rotate_y);
        assert!(!caps.rotate_x);
        assert!(!caps.nonuniform_scale);
    }

    #[test]
    fn pile_translation_only() {
        let caps = TransformCapabilities::world_item_pile();
        assert!(caps.translate_x);
        assert!(!caps.rotate_y);
    }
}
