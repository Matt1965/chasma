//! Built-in render pivot corrections for GLBs exported off-origin (ADR-096 BP-CLEANUP).
//!
//! [`super::catalog::BuildingDefinition::model_local_offset`] takes precedence. When zero,
//! known render keys may supply a documented correction before catalog migration.

use bevy::prelude::*;

use super::catalog::BuildingDefinition;

/// Pivot offset keyed by render asset stem (`assets/buildings/{key}.glb`).
pub fn builtin_model_local_offset(render_key: &str) -> Option<Vec3> {
    match render_key {
        // barn.glb scene roots were exported with Blender world translations (~-7, +19) XZ.
        "barn" => Some(Vec3::new(7.05, 0.35, -18.65)),
        _ => None,
    }
}

/// Effective model offset: explicit catalog data, else built-in correction for the render key.
pub fn effective_model_local_offset(definition: &BuildingDefinition) -> Vec3 {
    if definition.model_local_offset != Vec3::ZERO {
        return definition.model_local_offset;
    }
    definition
        .render_key
        .0
        .as_deref()
        .and_then(builtin_model_local_offset)
        .unwrap_or(Vec3::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCategoryId, BuildingDefinitionId, BuildingRenderKey, FootprintSpec,
    };

    fn barn_definition() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("barn"),
            "Barn",
            BuildingCategoryId::new("storage"),
            BuildingRenderKey::reserved("barn"),
            BuildingRenderKey::reserved("barn"),
            400,
            90.0,
            FootprintSpec::Rectangle {
                width_meters: 8.0,
                depth_meters: 6.0,
            },
            35.0,
            true,
        )
    }

    #[test]
    fn barn_builtin_offset_applies_when_catalog_offset_zero() {
        let offset = effective_model_local_offset(&barn_definition());
        assert!(offset.x > 6.0);
        assert!(offset.z < -15.0);
    }

    #[test]
    fn catalog_offset_overrides_builtin() {
        let definition = barn_definition().with_model_local_offset(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(
            effective_model_local_offset(&definition),
            Vec3::new(1.0, 2.0, 3.0)
        );
    }
}
