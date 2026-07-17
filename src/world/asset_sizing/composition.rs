//! Presentation model transform composition (ADR-097 DT1).
//!
//! Model-local offset is a **visual pivot correction** only. It does not alter authoritative
//! placement, collision centers, occupancy, or building anchors.

use bevy::prelude::*;

use crate::world::authoring_transform::{AuthoringScale, QuantizedOrientation};
use crate::world::building::{building_has_model_correction, effective_model_local_offset};
use crate::world::{BuildingDefinition, DoodadDefinition, UnitDefinition};

/// Uniform baseline render scale for units (no instance override in DT1).
pub fn unit_baseline_render_scale(definition: &UnitDefinition) -> f32 {
    definition
        .asset_sizing
        .resolved_baseline_scale()
        .uniform_value()
        .unwrap_or(definition.render_scale)
}

/// Non-uniform baseline render scale for doodad definitions.
pub fn doodad_baseline_render_scale(definition: &DoodadDefinition) -> Vec3 {
    definition.asset_sizing.resolved_baseline_scale().to_vec3()
}

/// Final doodad render scale = definition baseline × instance placement scale.
pub fn doodad_final_render_scale(definition: &DoodadDefinition, instance_scale: Vec3) -> Vec3 {
    doodad_baseline_render_scale(definition) * instance_scale
}

/// Uniform baseline render scale for building model presentation.
pub fn building_baseline_render_scale(definition: &BuildingDefinition) -> f32 {
    definition
        .asset_sizing
        .resolved_baseline_scale()
        .uniform_value()
        .unwrap_or(1.0)
}

/// Model orientation correction from catalog sizing (YXZ Euler).
pub fn sizing_rotation_correction(rotation: QuantizedOrientation) -> Quat {
    rotation.to_quat()
}

/// Combined model-local offset: explicit sizing offset when non-zero, else building builtin.
pub fn building_effective_model_offset(definition: &BuildingDefinition) -> Vec3 {
    if definition.asset_sizing.model_local_offset_meters != Vec3::ZERO {
        definition.asset_sizing.model_local_offset_meters
    } else {
        effective_model_local_offset(definition)
    }
}

pub fn building_uses_model_child(definition: &BuildingDefinition) -> bool {
    building_has_model_correction(definition)
        || definition.asset_sizing.model_local_offset_meters != Vec3::ZERO
        || building_baseline_render_scale(definition) != 1.0
}

/// Scale applied to building model child (baseline × dev instance uniform scale).
pub fn building_model_child_scale(
    definition: &BuildingDefinition,
    instance_uniform_scale: f32,
) -> Vec3 {
    Vec3::splat(building_baseline_render_scale(definition) * instance_uniform_scale)
}

/// Local child transform for building GLB (offset + sizing rotation + baseline scale).
pub fn building_model_child_local_transform(
    definition: &BuildingDefinition,
    instance_uniform_scale: f32,
) -> Transform {
    Transform {
        translation: building_effective_model_offset(definition),
        rotation: sizing_rotation_correction(definition.asset_sizing.rotation_correction),
        scale: building_model_child_scale(definition, instance_uniform_scale),
    }
}

/// Visual-vs-collision mismatch diagnostic for doodads (collision unchanged until DT2).
pub fn doodad_visual_collision_mismatch_warning(definition: &DoodadDefinition) -> Option<String> {
    if !definition.blocks_movement {
        return None;
    }
    let visual = doodad_baseline_render_scale(definition);
    let max_visual = visual.x.max(visual.y).max(visual.z);
    let collision_radius = definition.block_radius_meters;
    if collision_radius <= 0.0 {
        return None;
    }
    let ratio = max_visual.max(1.0) / collision_radius.max(0.01);
    if ratio > 2.5 || ratio < 0.4 {
        Some(format!(
            "visual baseline scale ({max_visual:.2}×) differs from block radius ({collision_radius:.2} m) — collision updates in DT2"
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
    fn doodad_final_scale_multiplies_baseline_and_instance() {
        let mut sizing = AssetSizingDefinition::default();
        sizing.calculated_baseline_scale =
            Some(AuthoringScale::from_non_uniform_f32(2.0, 1.0, 1.5).unwrap());
        let definition = DoodadDefinition::new(
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
        let mut definition = definition;
        definition.asset_sizing = sizing;
        let final_scale = doodad_final_render_scale(&definition, Vec3::splat(1.1));
        assert!((final_scale.x - 2.2).abs() < 0.01);
    }
}
