//! Equipment visual fit tuning (post-CG6 replacement for body masking).
//!
//! Skinned overlays bake [`EquipmentVisualMapping::fit_scale`] and
//! [`EquipmentVisualMapping::fit_offset`] into equipment GLBs via
//! `scripts/author_equipment_fit.py`. Runtime [`Transform::scale`] on skinned
//! equipment would desync joint-bound meshes from the live unit skeleton.

use bevy::prelude::*;

use super::presentation::EquipmentPresentationMode;

/// Default fit scale — preserves legacy presentation when unset.
pub const DEFAULT_EQUIPMENT_FIT_SCALE: f32 = 1.0;

/// Whether authored fit is applied offline rather than at spawn.
pub fn skinned_fit_is_baked_offline(mode: EquipmentPresentationMode) -> bool {
    matches!(mode, EquipmentPresentationMode::SkinnedOverlay)
}

/// Optional rigid-only runtime composition (grip tuning + fit). Skinned overlays ignore this.
pub fn effective_rigid_local_scale(local_scale: Vec3, fit_scale: f32) -> Vec3 {
    local_scale * fit_scale
}

/// Optional rigid-only runtime composition (grip tuning + fit). Skinned overlays ignore this.
pub fn effective_rigid_local_translation(local_translation: Vec3, fit_offset: Vec3) -> Vec3 {
    local_translation + fit_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fit_preserves_rigid_transform() {
        let local = Vec3::new(0.1, 0.2, 0.3);
        assert_eq!(
            effective_rigid_local_scale(Vec3::ONE, DEFAULT_EQUIPMENT_FIT_SCALE),
            Vec3::ONE
        );
        assert_eq!(
            effective_rigid_local_translation(local, Vec3::ZERO),
            local
        );
    }

    #[test]
    fn skinned_overlay_fit_is_baked_offline() {
        assert!(skinned_fit_is_baked_offline(EquipmentPresentationMode::SkinnedOverlay));
        assert!(!skinned_fit_is_baked_offline(EquipmentPresentationMode::RigidAttachment));
    }

    #[test]
    fn rigid_fit_composes_with_grip_tuning() {
        assert_eq!(
            effective_rigid_local_scale(Vec3::new(1.0, 1.0, 1.0), 1.04),
            Vec3::splat(1.04)
        );
        assert_eq!(
            effective_rigid_local_translation(Vec3::new(0.0, 0.1, 0.0), Vec3::new(0.0, 0.01, 0.0)),
            Vec3::new(0.0, 0.11, 0.0)
        );
    }
}
