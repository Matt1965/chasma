//! Presentation transform composition (ADR-097 DT1, ADR-126/128 AT2, ADR-129 AT3).
//!
//! # Visual scale (exactly one composition)
//!
//! ```text
//! definition baseline  (catalog: baked import+desired, or explicit)
//!         ×
//! instance scale       (placement; default 1)
//!         =
//! presentation Transform.scale
//! ```
//!
//! Offline, import measurement and catalog desired meters produce the single
//! definition baseline (`calculated_baseline_scale` / `explicit_baseline_scale`).
//! Runtime does **not** multiply a separate "import baseline" and "catalog baseline".
//!
//! # Gameplay coupling (AT3)
//!
//! - **Doodads:** collision / pick / occupancy = authored meters × (baseline × instance)_xz
//! - **Buildings:** occupancy / pick / portals = footprint meters × instance (baseline validated ≈ footprint)
//!
//! Model-local offset and rotation correction remain definition-owned **visual** corrections.
//! They do not alter occupancy anchors.
//!
//! ECS `Transform` is presentation-only.

use bevy::prelude::*;

use crate::world::authoring_transform::{AuthoringScale, QuantizedOrientation};
use crate::world::building::effective_model_local_offset;
use crate::world::{BuildingDefinition, DoodadDefinition, UnitDefinition};

use super::definition::AssetSizingDefinition;

// ---------------------------------------------------------------------------
// Core compose (AT2)
// ---------------------------------------------------------------------------

/// Catalog-owned definition baseline scale (baked from import+desired, or explicit).
pub fn definition_visual_baseline(sizing: &AssetSizingDefinition) -> AuthoringScale {
    sizing.resolved_baseline_scale()
}

/// Presentation baseline applied to raw GLB vertices at runtime.
///
/// Import may quantize baseline against unit-corrected bounds; this divides by
/// [`AssetSizingDefinition::source_bounds_unit_divisor`] so mm/cm exports still
/// reach desired meters.
pub fn definition_presentation_baseline_vec3(sizing: &AssetSizingDefinition) -> Vec3 {
    let baseline = sizing.resolved_baseline_scale().to_vec3();
    let divisor = sizing.source_bounds_unit_divisor;
    if divisor > 1.0 + f32::EPSILON {
        baseline / divisor
    } else {
        baseline
    }
}

/// Compose the single presentation scale: `definition_baseline × instance_scale`.
pub fn compose_visual_scale(baseline: AuthoringScale, instance: AuthoringScale) -> Vec3 {
    baseline.to_vec3() * instance.to_vec3()
}

/// Compose presentation scale from vec3 baselines (supports sub-quantized baselines).
pub fn compose_visual_scale_vec3(baseline: Vec3, instance: Vec3) -> Vec3 {
    baseline * instance
}

/// Building visual scale (uniform baseline × uniform instance).
pub fn building_visual_scale(
    definition: &BuildingDefinition,
    instance_uniform_scale: f32,
) -> Vec3 {
    let baseline = definition_presentation_baseline_vec3(&definition.asset_sizing);
    let instance = AuthoringScale::from_uniform_f32(instance_uniform_scale)
        .unwrap_or_else(|_| AuthoringScale::uniform_one())
        .to_vec3();
    compose_visual_scale_vec3(baseline, instance)
}

/// Doodad visual scale (baseline × instance; non-uniform allowed).
pub fn doodad_visual_scale(definition: &DoodadDefinition, instance_scale: Vec3) -> Vec3 {
    let baseline = definition_presentation_baseline_vec3(&definition.asset_sizing);
    compose_visual_scale_vec3(baseline, instance_scale)
}

/// Unit visual scale (definition baseline only; no instance scale today).
pub fn unit_visual_scale(definition: &UnitDefinition) -> Vec3 {
    let baseline = definition_presentation_baseline_vec3(&definition.asset_sizing);
    // Prefer metric baseline; fall back to legacy render_scale when sizing missing.
    if definition.asset_sizing.is_missing_sizing_data()
        && definition.asset_sizing.calculated_baseline_scale.is_none()
        && definition.asset_sizing.explicit_baseline_scale.is_none()
    {
        return Vec3::splat(definition.render_scale);
    }
    compose_visual_scale_vec3(baseline, Vec3::ONE)
}

// ---------------------------------------------------------------------------
// Backward-compatible aliases (same composition; prefer *_visual_scale)
// ---------------------------------------------------------------------------

/// Uniform baseline render scale for units.
pub fn unit_baseline_render_scale(definition: &UnitDefinition) -> f32 {
    unit_visual_scale(definition).x
}

/// Non-uniform baseline only (definition layer — not final presentation).
pub fn doodad_baseline_render_scale(definition: &DoodadDefinition) -> Vec3 {
    definition_presentation_baseline_vec3(&definition.asset_sizing)
}

/// Final doodad presentation scale = baseline × instance.
pub fn doodad_final_render_scale(definition: &DoodadDefinition, instance_scale: Vec3) -> Vec3 {
    doodad_visual_scale(definition, instance_scale)
}

/// Uniform definition baseline for buildings.
pub fn building_baseline_render_scale(definition: &BuildingDefinition) -> f32 {
    definition_presentation_baseline_vec3(&definition.asset_sizing).x
}

/// Scale applied to building model child (composed visual scale).
pub fn building_model_child_scale(
    definition: &BuildingDefinition,
    instance_uniform_scale: f32,
) -> Vec3 {
    building_visual_scale(definition, instance_uniform_scale)
}

// ---------------------------------------------------------------------------
// Rotation / offset (definition corrections)
// ---------------------------------------------------------------------------

/// Model orientation correction from catalog sizing (YXZ Euler).
pub fn sizing_rotation_correction(rotation: QuantizedOrientation) -> Quat {
    rotation.to_quat()
}

/// Authoritative pivot offset (meters) from AT1 sizing authority.
pub fn building_effective_model_offset(definition: &BuildingDefinition) -> Vec3 {
    if definition.asset_sizing.model_local_offset_meters != Vec3::ZERO {
        definition.asset_sizing.model_local_offset_meters
    } else {
        effective_model_local_offset(definition)
    }
}

/// Whether the building needs a model child (offset, rotation correction, or non-1 visual scale).
pub fn building_uses_model_child(definition: &BuildingDefinition) -> bool {
    let baseline = definition_presentation_baseline_vec3(&definition.asset_sizing);
    building_effective_model_offset(definition) != Vec3::ZERO
        || definition.asset_sizing.rotation_correction != QuantizedOrientation::IDENTITY
        || (baseline - Vec3::ONE).length_squared() > f32::EPSILON
}

/// Local child transform for building GLB: offset + definition rotation + composed scale.
///
/// Parent anchor carries placement pose only (no yaw correction) to avoid double-applying
/// `asset_sizing.rotation_correction` / legacy yaw mirrors.
pub fn building_model_child_local_transform(
    definition: &BuildingDefinition,
    instance_uniform_scale: f32,
) -> Transform {
    Transform {
        translation: building_effective_model_offset(definition),
        rotation: sizing_rotation_correction(definition.asset_sizing.rotation_correction),
        scale: building_visual_scale(definition, instance_uniform_scale),
    }
}

/// Visual-vs-collision mismatch diagnostic for doodads (meters vs meters, instance = 1).
pub fn doodad_visual_collision_mismatch_warning(definition: &DoodadDefinition) -> Option<String> {
    if !definition.blocks_movement {
        return None;
    }
    let collision_radius = definition
        .block_radius_meters
        .max(definition.base_collision_radius_x_meters)
        .max(definition.base_collision_radius_z_meters);
    if collision_radius <= 0.0 {
        return None;
    }
    let visual_xz = definition
        .asset_sizing
        .approximate_final_dimensions_meters()
        .map(|d| d.width_meters.max(d.depth_meters) * 0.5)
        .or_else(|| {
            let source = definition.asset_sizing.authoritative_source_dimensions()?;
            let baseline = definition_presentation_baseline_vec3(&definition.asset_sizing);
            Some((source.width_meters * baseline.x).max(source.depth_meters * baseline.z) * 0.5)
        })?;
    if visual_xz <= 0.0 {
        return None;
    }
    let ratio = visual_xz.max(0.01) / collision_radius.max(0.01);
    if ratio > 2.5 || ratio < 0.4 {
        Some(format!(
            "visual XZ half-extent (~{visual_xz:.2} m) differs from collision radius ({collision_radius:.2} m) — author collision meters to match desired size"
        ))
    } else {
        None
    }
}

/// Navigable building visual size vs authored footprint (ADR-126 preferred: validate, don't auto-scale footprint by baseline).
pub fn building_visual_footprint_mismatch_warning(
    definition: &BuildingDefinition,
) -> Option<String> {
    use crate::world::authoring_transform::BuildingTransformSafetyClass;
    if definition.transform_safety_class != BuildingTransformSafetyClass::Navigable {
        return None;
    }
    let final_dims = definition.asset_sizing.approximate_final_dimensions_meters()?;
    let (fw, fd) = match &definition.footprint {
        crate::world::FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => (*width_meters, *depth_meters),
        crate::world::FootprintSpec::Circle { radius_meters } => {
            let d = radius_meters * 2.0;
            (d, d)
        }
        crate::world::FootprintSpec::MeshDerived => return None,
    };
    if fw <= 0.0 || fd <= 0.0 {
        return None;
    }
    let width_delta = (final_dims.width_meters - fw).abs();
    let depth_delta = (final_dims.depth_meters - fd).abs();
    if width_delta > fw * 0.25 || depth_delta > fd * 0.25 {
        Some(format!(
            "visual size ({:.2}×{:.2} m) diverges from footprint ({fw:.2}×{fd:.2} m) — keep desired meters ≈ footprint (ADR-126)",
            final_dims.width_meters, final_dims.depth_meters
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::asset_sizing::AssetSizingDefinition;
    use crate::world::authoring_transform::AuthoringScale;
    use crate::world::{DoodadDefinition, DoodadDefinitionId, DoodadKind, DoodadRenderKey};

    #[test]
    fn compose_is_baseline_times_instance() {
        let baseline = AuthoringScale::from_non_uniform_f32(2.0, 1.0, 1.5).unwrap();
        let instance = AuthoringScale::from_uniform_f32(1.1).unwrap();
        let composed = compose_visual_scale(baseline, instance);
        assert!((composed.x - 2.2).abs() < 0.01);
        assert!((composed.y - 1.1).abs() < 0.01);
        assert!((composed.z - 1.65).abs() < 0.01);
    }

    #[test]
    fn doodad_final_scale_multiplies_baseline_and_instance() {
        let mut sizing = AssetSizingDefinition::default();
        sizing.calculated_baseline_scale =
            Some(AuthoringScale::from_non_uniform_f32(2.0, 1.0, 1.5).unwrap());
        let mut definition = DoodadDefinition::new(
            DoodadDefinitionId::new("chest"),
            DoodadKind::Ruin,
            "Chest",
            1.0,
            0.5,
            2.0,
            None,
            None,
            None,
            true,
            DoodadRenderKey::reserved("chest"),
        );
        definition.asset_sizing = sizing;
        let final_scale = doodad_visual_scale(&definition, Vec3::splat(1.1));
        assert!((final_scale.x - 2.2).abs() < 0.01);
    }

    #[test]
    fn building_visual_scale_is_uniform_product() {
        use crate::world::{
            BuildingCategoryId, BuildingDefinitionId, BuildingRenderKey, FootprintSpec,
        };
        let mut def = BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Hut",
            BuildingCategoryId::new("residential"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut"),
            100,
            10.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        );
        def.asset_sizing.calculated_baseline_scale =
            Some(AuthoringScale::from_uniform_f32(2.0).unwrap());
        let scale = building_visual_scale(&def, 1.5);
        assert!((scale.x - 3.0).abs() < 0.01);
        assert_eq!(scale.x, scale.y);
        assert_eq!(scale.y, scale.z);
    }
}
