//! AT1: catalog-owned asset sizing authority (ADR-126 / ADR-127).
//!
//! `AssetSizingDefinition` is the single authoritative home for desired meters, measured
//! source bounds, baked baseline scale, pivot offset, and import rotation correction.
//! Legacy building fields (`model_local_offset`, `model_yaw_correction_degrees`) remain as
//! mirrors for backward compatibility. This module does **not** change runtime transform
//! composition — it only migrates and exposes definition data.

use bevy::prelude::*;

use crate::world::authoring_transform::QuantizedOrientation;
use crate::world::building::builtin_model_local_offset;
use crate::world::BuildingDefinition;

use super::definition::{AssetSizingDefinition, SizingMigrationState, SourceDimensions};

/// Issues found when validating catalog sizing authority (AT1 diagnostics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizingAuthorityIssue {
    MissingSizingData { definition_id: String },
    LegacyExplicitScale { definition_id: String },
    DualPivotMismatch { definition_id: String },
    DualYawMismatch { definition_id: String },
}

impl std::fmt::Display for SizingAuthorityIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSizingData { definition_id } => write!(
                f,
                "AT1: `{definition_id}` has MissingSizingData — add Desired meters or bake baseline"
            ),
            Self::LegacyExplicitScale { definition_id } => write!(
                f,
                "AT1: `{definition_id}` still on LegacyExplicitScale — migrate to Desired meters"
            ),
            Self::DualPivotMismatch { definition_id } => write!(
                f,
                "AT1: `{definition_id}` pivot mismatch between asset_sizing and model_local_offset"
            ),
            Self::DualYawMismatch { definition_id } => write!(
                f,
                "AT1: `{definition_id}` yaw mismatch between asset_sizing.rotation_correction and model_yaw_correction_degrees"
            ),
        }
    }
}

impl AssetSizingDefinition {
    /// Authoritative pivot correction (meters). Prefer this over legacy building fields.
    pub fn authoritative_pivot_offset_meters(&self) -> Vec3 {
        self.model_local_offset_meters
    }

    /// Authoritative import rotation correction.
    pub fn authoritative_rotation_correction(&self) -> QuantizedOrientation {
        self.rotation_correction
    }

    /// Measured or explicit source dimensions (meters).
    pub fn authoritative_source_dimensions(&self) -> Option<SourceDimensions> {
        self.resolved_source_bounds()
    }

    /// Baked / explicit baseline scale used by presentation (AT2+ applies more widely).
    pub fn authoritative_baseline_scale(&self) -> crate::world::authoring_transform::AuthoringScale {
        self.resolved_baseline_scale()
    }

    /// Approximate final visual size after baseline (source × baseline). Instance scale not included.
    pub fn approximate_final_dimensions_meters(&self) -> Option<SourceDimensions> {
        let source = self.resolved_source_bounds()?;
        let scale = self.resolved_baseline_scale().to_vec3();
        Some(SourceDimensions {
            width_meters: source.width_meters * scale.x,
            height_meters: source.height_meters * scale.y,
            depth_meters: source.depth_meters * scale.z,
        })
    }

    pub fn is_metric_configured(&self) -> bool {
        matches!(self.migration_state, SizingMigrationState::MetricConfigured)
    }

    pub fn is_missing_sizing_data(&self) -> bool {
        matches!(self.migration_state, SizingMigrationState::MissingSizingData)
    }
}

/// Migrate legacy building correction fields and builtins into `asset_sizing`, then sync mirrors.
///
/// Precedence for pivot:
/// 1. Non-zero `asset_sizing.model_local_offset_meters`
/// 2. Non-zero legacy `model_local_offset`
/// 3. Builtin render-key correction (temporary until AT2 content bake)
///
/// Precedence for rotation:
/// 1. Non-identity `asset_sizing.rotation_correction`
/// 2. Legacy `model_yaw_correction_degrees` (yaw only → sizing; preserves pitch/roll if already set)
///
/// After this runs, legacy fields mirror `asset_sizing` so old readers stay consistent.
/// Does not modify desired dimensions, source bounds, or baseline scale.
pub fn normalize_building_sizing_authority(definition: &mut BuildingDefinition) {
    let pivot = if definition.asset_sizing.model_local_offset_meters != Vec3::ZERO {
        definition.asset_sizing.model_local_offset_meters
    } else if definition.model_local_offset != Vec3::ZERO {
        definition.model_local_offset
    } else {
        definition
            .render_key
            .0
            .as_deref()
            .and_then(builtin_model_local_offset)
            .unwrap_or(Vec3::ZERO)
    };
    definition.asset_sizing.model_local_offset_meters = pivot;
    definition.model_local_offset = pivot;

    let sizing_rot = definition.asset_sizing.rotation_correction;
    let legacy_yaw = definition.model_yaw_correction_degrees;
    if sizing_rot == QuantizedOrientation::IDENTITY && legacy_yaw.abs() > f32::EPSILON {
        if let Ok(rot) = QuantizedOrientation::from_degrees(legacy_yaw, 0.0, 0.0) {
            definition.asset_sizing.rotation_correction = rot;
        }
    }
    definition.model_yaw_correction_degrees =
        definition.asset_sizing.rotation_correction.yaw_degrees();
}

/// Sync legacy building mirrors from authoritative `asset_sizing` only (no builtin injection).
pub fn sync_building_legacy_mirrors_from_sizing(definition: &mut BuildingDefinition) {
    definition.model_local_offset = definition.asset_sizing.model_local_offset_meters;
    definition.model_yaw_correction_degrees =
        definition.asset_sizing.rotation_correction.yaw_degrees();
}

/// Validate that building legacy mirrors match `asset_sizing` after normalization.
pub fn validate_building_sizing_authority(
    definition: &BuildingDefinition,
) -> Vec<SizingAuthorityIssue> {
    let id = definition.id.as_str().to_string();
    let mut issues = validate_sizing_migration_state(definition.id.as_str(), &definition.asset_sizing);

    let pivot_delta =
        (definition.model_local_offset - definition.asset_sizing.model_local_offset_meters).length();
    if pivot_delta > 1e-4 {
        issues.push(SizingAuthorityIssue::DualPivotMismatch {
            definition_id: id.clone(),
        });
    }

    let yaw_delta = (definition.model_yaw_correction_degrees
        - definition.asset_sizing.rotation_correction.yaw_degrees())
    .abs();
    if yaw_delta > 0.01 {
        issues.push(SizingAuthorityIssue::DualYawMismatch {
            definition_id: id,
        });
    }

    issues
}

pub fn validate_sizing_migration_state(
    definition_id: &str,
    sizing: &AssetSizingDefinition,
) -> Vec<SizingAuthorityIssue> {
    let mut issues = Vec::new();
    match sizing.migration_state {
        SizingMigrationState::MissingSizingData => {
            issues.push(SizingAuthorityIssue::MissingSizingData {
                definition_id: definition_id.to_string(),
            });
        }
        SizingMigrationState::LegacyExplicitScale => {
            issues.push(SizingAuthorityIssue::LegacyExplicitScale {
                definition_id: definition_id.to_string(),
            });
        }
        SizingMigrationState::MetricConfigured => {}
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCategoryId, BuildingDefinitionId, BuildingRenderKey, FootprintSpec,
    };

    fn bare_barn() -> BuildingDefinition {
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
    fn normalize_injects_builtin_into_asset_sizing_and_mirrors() {
        let mut def = bare_barn();
        assert_eq!(def.asset_sizing.model_local_offset_meters, Vec3::ZERO);
        normalize_building_sizing_authority(&mut def);
        assert!(def.asset_sizing.model_local_offset_meters.x > 6.0);
        assert_eq!(def.model_local_offset, def.asset_sizing.model_local_offset_meters);
        assert!(validate_building_sizing_authority(&def).iter().all(|i| {
            !matches!(
                i,
                SizingAuthorityIssue::DualPivotMismatch { .. }
                    | SizingAuthorityIssue::DualYawMismatch { .. }
            )
        }));
    }

    #[test]
    fn asset_sizing_wins_over_legacy_when_both_set() {
        let mut def = bare_barn()
            .with_model_local_offset(Vec3::new(1.0, 0.0, 0.0));
        // Simulate divergent legacy after builder (builder now syncs both — force diverge).
        def.model_local_offset = Vec3::new(9.0, 0.0, 0.0);
        def.asset_sizing.model_local_offset_meters = Vec3::new(2.0, 0.0, 0.0);
        normalize_building_sizing_authority(&mut def);
        assert_eq!(def.asset_sizing.model_local_offset_meters, Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(def.model_local_offset, Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn legacy_yaw_migrates_into_sizing() {
        let mut def = bare_barn();
        def.model_yaw_correction_degrees = 90.0;
        normalize_building_sizing_authority(&mut def);
        assert!((def.asset_sizing.rotation_correction.yaw_degrees() - 90.0).abs() < 0.01);
        assert!((def.model_yaw_correction_degrees - 90.0).abs() < 0.01);
    }
}
