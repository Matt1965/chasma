//! Animation asset validation (A6 / D6). Presentation-only — never mutates gameplay.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::{
    AnimationClipKey, AnimationProfile, AnimationProfileCatalog, UnitCatalog, UnitDefinition,
    UnitDefinitionId, WeaponCatalog,
};

use super::locomotion_polish::MODEL_FORWARD_AXIS;

/// Validation severity (A6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

/// One validation finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub code: &'static str,
    pub message: String,
}

/// Per-definition validation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionValidationReport {
    pub definition_id: UnitDefinitionId,
    pub issues: Vec<ValidationIssue>,
}

impl DefinitionValidationReport {
    pub fn push(
        &mut self,
        severity: ValidationSeverity,
        code: &'static str,
        message: impl Into<String>,
    ) {
        self.issues.push(ValidationIssue {
            severity,
            code,
            message: message.into(),
        });
    }

    pub fn has_error(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == ValidationSeverity::Error)
    }

    pub fn worst_severity(&self) -> Option<ValidationSeverity> {
        self.issues
            .iter()
            .map(|issue| issue.severity)
            .max_by_key(|s| match s {
                ValidationSeverity::Error => 2,
                ValidationSeverity::Warning => 1,
                ValidationSeverity::Info => 0,
            })
    }
}

/// Cached validation reports keyed by definition (A6).
#[derive(Resource, Debug, Default, Clone)]
pub struct AnimationValidationIndex {
    pub reports: HashMap<UnitDefinitionId, DefinitionValidationReport>,
    pub logged_keys: std::collections::HashSet<String>,
}

impl AnimationValidationIndex {
    pub fn report_for(
        &self,
        definition_id: &UnitDefinitionId,
    ) -> Option<&DefinitionValidationReport> {
        self.reports.get(definition_id)
    }

    pub fn log_new_issues(&mut self, report: &DefinitionValidationReport) {
        for issue in &report.issues {
            let key = format!(
                "{}:{}:{}",
                report.definition_id.as_str(),
                issue.code,
                issue.message
            );
            if self.logged_keys.insert(key) {
                match issue.severity {
                    ValidationSeverity::Error => {
                        error!("unit animation validation: {}", issue.message)
                    }
                    ValidationSeverity::Warning => {
                        warn!("unit animation validation: {}", issue.message)
                    }
                    ValidationSeverity::Info => {
                        info!("unit animation validation: {}", issue.message)
                    }
                }
            }
        }
    }

    pub fn missing_profile_count(&self) -> u32 {
        self.reports
            .values()
            .filter(|report| report.issues.iter().any(|i| i.code == "missing_profile"))
            .count() as u32
    }

    pub fn missing_clip_count(&self) -> u32 {
        self.reports
            .values()
            .flat_map(|report| report.issues.iter())
            .filter(|issue| issue.code == "missing_required_clip")
            .count() as u32
    }
}

/// Validate profile + glTF clip availability for one definition (A6).
pub fn validate_definition_animation_assets(
    definition: &UnitDefinition,
    profile: Option<&AnimationProfile>,
    gltf: Option<&Gltf>,
    weapon_clip_name: Option<&str>,
) -> DefinitionValidationReport {
    let mut report = DefinitionValidationReport {
        definition_id: definition.id.clone(),
        issues: Vec::new(),
    };

    let Some(profile) = profile else {
        if definition.animation_profile_id.is_some() {
            report.push(
                ValidationSeverity::Error,
                "missing_profile",
                format!(
                    "unit `{}` references missing animation profile",
                    definition.id.as_str()
                ),
            );
        } else {
            report.push(
                ValidationSeverity::Info,
                "static_model",
                format!(
                    "unit `{}` has no animation profile (static model)",
                    definition.id.as_str()
                ),
            );
        }
        return report;
    };

    if !profile.enabled {
        report.push(
            ValidationSeverity::Info,
            "profile_disabled",
            format!(
                "profile `{}` disabled for unit `{}`",
                profile.id.as_str(),
                definition.id.as_str()
            ),
        );
        return report;
    }

    let Some(gltf) = gltf else {
        report.push(
            ValidationSeverity::Error,
            "missing_gltf",
            format!(
                "unit `{}` glTF not loaded for animation validation",
                definition.id.as_str()
            ),
        );
        return report;
    };

    validate_required_locomotion_clips(profile, gltf, &mut report);
    validate_optional_clips(profile, gltf, &mut report);
    validate_weapon_clip(gltf, weapon_clip_name, &mut report);
    validate_forward_axis_convention(&mut report);

    if gltf.named_animations.is_empty() {
        report.push(
            ValidationSeverity::Error,
            "no_clips",
            format!(
                "unit `{}` glTF contains no named animations",
                definition.id.as_str()
            ),
        );
    }

    report
}

fn validate_required_locomotion_clips(
    profile: &AnimationProfile,
    gltf: &Gltf,
    report: &mut DefinitionValidationReport,
) {
    let idle = profile.resolve_clip_name(AnimationClipKey::Idle);
    match idle {
        Some((name, _)) if gltf.named_animations.contains_key(name) => {}
        Some((name, _)) => report.push(
            ValidationSeverity::Error,
            "missing_required_clip",
            format!("required Idle clip `{name}` missing in glTF"),
        ),
        None => report.push(
            ValidationSeverity::Error,
            "missing_required_clip",
            "profile has no resolvable Idle clip".to_string(),
        ),
    }
}

fn validate_optional_clips(
    profile: &AnimationProfile,
    gltf: &Gltf,
    report: &mut DefinitionValidationReport,
) {
    for (key, label, severity) in [
        (AnimationClipKey::Walk, "Walk", ValidationSeverity::Warning),
        (AnimationClipKey::Run, "Run", ValidationSeverity::Warning),
        (
            AnimationClipKey::TurnLeft,
            "TurnLeft",
            ValidationSeverity::Warning,
        ),
        (
            AnimationClipKey::TurnRight,
            "TurnRight",
            ValidationSeverity::Warning,
        ),
    ] {
        if let Some((name, _)) = profile.resolve_clip_name(key) {
            if !gltf.named_animations.contains_key(name) {
                report.push(
                    severity,
                    "missing_optional_clip",
                    format!("optional {label} clip `{name}` missing in glTF (fallback applies)"),
                );
            }
        }
    }

    if let Some(name) = profile.resolve_death_clip_name() {
        if !gltf.named_animations.contains_key(name) {
            report.push(
                ValidationSeverity::Warning,
                "missing_death_clip",
                format!("death clip `{name}` missing — freeze-pose fallback"),
            );
        }
    }

    if let Some(name) = profile.resolve_hit_reaction_clip_name() {
        if !gltf.named_animations.contains_key(name) {
            report.push(
                ValidationSeverity::Warning,
                "missing_hit_clip",
                format!("hit reaction clip `{name}` missing — idle fallback"),
            );
        }
    }

    if profile.layering_split_bone().is_none() {
        report.push(
            ValidationSeverity::Warning,
            "layering_unsupported",
            format!(
                "profile `{}` has no upper_body_split_bone — full-body exclusive playback",
                profile.id.as_str()
            ),
        );
    }
}

fn validate_weapon_clip(
    gltf: &Gltf,
    weapon_clip_name: Option<&str>,
    report: &mut DefinitionValidationReport,
) {
    let Some(name) = weapon_clip_name.filter(|value| !value.is_empty()) else {
        report.push(
            ValidationSeverity::Warning,
            "blank_attack_clip",
            "default weapon has blank attack animation key",
        );
        return;
    };
    if !gltf.named_animations.contains_key(name) {
        report.push(
            ValidationSeverity::Warning,
            "missing_attack_clip",
            format!("attack clip `{name}` missing in glTF — idle fallback"),
        );
    }
}

fn validate_forward_axis_convention(report: &mut DefinitionValidationReport) {
    report.push(
        ValidationSeverity::Info,
        "model_forward_axis",
        format!(
            "model forward axis convention: {:?} (see docs/animation-authoring.md)",
            MODEL_FORWARD_AXIS
        ),
    );
}

/// Validate catalog entries at graph-build time (A6).
pub fn validate_catalog_animation_assets(
    catalog: &UnitCatalog,
    profiles: &AnimationProfileCatalog,
    weapons: &WeaponCatalog,
    gltfs: &Assets<Gltf>,
    gltf_handles: &HashMap<UnitDefinitionId, Handle<Gltf>>,
) -> AnimationValidationIndex {
    let mut index = AnimationValidationIndex::default();
    for definition in catalog.definitions() {
        let profile = definition
            .animation_profile_id
            .as_ref()
            .and_then(|id| profiles.get(id));
        let gltf = gltf_handles
            .get(&definition.id)
            .and_then(|handle| gltfs.get(handle));
        let weapon_clip = weapons
            .get(&definition.default_weapon_id)
            .map(|weapon| weapon.animation_key.as_str());
        let report = validate_definition_animation_assets(definition, profile, gltf, weapon_clip);
        index.log_new_issues(&report);
        index.reports.insert(definition.id.clone(), report);
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{AnimationProfileId, UnitRenderKey, WeaponDefinitionId};

    fn sample_definition(profile_id: Option<&str>) -> UnitDefinition {
        let mut definition = UnitDefinition::new(
            UnitDefinitionId::new("wolf"),
            "Wolf",
            "Wild",
            2,
            5,
            5,
            4,
            6,
            3,
            7,
            2,
            3,
            26.5,
            "Elite",
            4.0,
            0.6,
            40.0,
            WeaponDefinitionId::new("weapon_wolf_bite"),
            true,
            UnitRenderKey::reserved("wolf"),
        );
        definition.animation_profile_id = profile_id.map(AnimationProfileId::new);
        definition
    }

    #[test]
    fn missing_profile_is_error() {
        let definition = sample_definition(Some("missing"));
        let report = validate_definition_animation_assets(&definition, None, None, None);
        assert!(report.has_error());
    }

    #[test]
    fn static_model_is_info() {
        let definition = sample_definition(None);
        let report = validate_definition_animation_assets(&definition, None, None, None);
        assert!(report.issues.iter().any(|i| i.code == "static_model"));
    }

    #[test]
    fn issue_logging_deduplicated() {
        let mut index = AnimationValidationIndex::default();
        let report = DefinitionValidationReport {
            definition_id: UnitDefinitionId::new("wolf"),
            issues: vec![ValidationIssue {
                severity: ValidationSeverity::Warning,
                code: "test",
                message: "once".to_string(),
            }],
        };
        index.log_new_issues(&report);
        index.log_new_issues(&report);
        assert_eq!(index.logged_keys.len(), 1);
    }
}
