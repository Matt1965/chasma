//! Animation asset validation (A6 / D6). Presentation-only — never mutates gameplay.

use std::collections::HashMap;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::world::{
    AnimationClipKey, AnimationProfile, UnitDefinition, UnitDefinitionId,
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

/// glTF load state for animation validation (defer until loaded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationGltfValidationState {
    /// Handle not registered yet, or asset server still loading dependencies.
    Pending,
    /// Asset server reported a failed load.
    LoadFailed,
    /// Load finished; `Gltf` asset data should be available.
    Ready,
}

/// Classify whether animation validation may run for a unit glTF handle.
pub fn animation_gltf_validation_state(
    handle: Option<&Handle<Gltf>>,
    asset_server: &AssetServer,
) -> AnimationGltfValidationState {
    let Some(handle) = handle else {
        return AnimationGltfValidationState::Pending;
    };
    match asset_server.get_load_state(handle) {
        Some(LoadState::Loaded) => AnimationGltfValidationState::Ready,
        Some(LoadState::Failed(_)) => AnimationGltfValidationState::LoadFailed,
        Some(LoadState::Loading) | Some(LoadState::NotLoaded) | None => {
            AnimationGltfValidationState::Pending
        }
    }
}

/// Validate profile + glTF clip availability for one definition (A6).
pub fn validate_definition_animation_assets(
    definition: &UnitDefinition,
    profile: Option<&AnimationProfile>,
    gltf_state: AnimationGltfValidationState,
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

    match gltf_state {
        AnimationGltfValidationState::Pending => return report,
        AnimationGltfValidationState::LoadFailed => {
            report.push(
                ValidationSeverity::Error,
                "missing_gltf",
                format!(
                    "unit `{}` glTF failed to load for animation validation",
                    definition.id.as_str()
                ),
            );
            return report;
        }
        AnimationGltfValidationState::Ready => {}
    }

    let Some(gltf) = gltf else {
        report.push(
            ValidationSeverity::Error,
            "missing_gltf",
            format!(
                "unit `{}` glTF load completed but asset data is unavailable",
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
        (
            AnimationClipKey::Swim,
            "Swim",
            ValidationSeverity::Warning,
        ),
        (
            AnimationClipKey::SwimIdle,
            "SwimIdle",
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
            "Chasma gameplay/model-forward convention is local {MODEL_FORWARD_AXIS:?}; \
             source GLB forward is not inferred from the asset — set Units rotation correction \
             columns when visual facing differs (see docs/animation-authoring.md)"
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{AnimationProfileId, UnitRenderKey, WeaponDefinitionId};

    fn sample_definition(profile_id: Option<&str>) -> UnitDefinition {
        let mut definition = UnitDefinition::new_test(
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

    /// Clip-rig rest jaw rotation on idle (closed mouth). SK bind stays open; merge fills this constant.
    const BUFOMORPH_IDLE_JAW_ROTATION: [f32; 4] = [
        0.666_896_16,
        0.745_150_7,
        -4.168_088_8e-8,
        -7.521_016_4e-7,
    ];

    /// SK mesh bind jaw (open modelling pose) — idle clips must not leave the mesh here at t=0.
    const BUFOMORPH_SK_BIND_JAW_ROTATION: [f32; 4] = [
        -0.860_101_3,
        -0.510_123_3,
        -2.66e-7,
        7.29e-7,
    ];

    #[test]
    #[cfg(feature = "dev")]
    fn bufomorph_idle_look_around_jaw_pose_matches_pack_authoring() {
        use std::path::PathBuf;

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/units/bufomorph.glb");
        if !path.is_file() {
            return;
        }
        let (document, buffers, _) = gltf::import(&path).expect("bufomorph glb");
        let sk_jaw = document
            .nodes()
            .find(|n| n.name() == Some("Bufomorph_JawUpper"))
            .expect("Bufomorph_JawUpper node");
        let sk_rot = sk_jaw.transform().decomposed().1;
        let sk_dot = sk_rot
            .iter()
            .zip(BUFOMORPH_SK_BIND_JAW_ROTATION)
            .map(|(a, b)| *a * b)
            .sum::<f32>();
        assert!(
            sk_dot.abs() > 0.999,
            "SK bind jaw should remain pack open pose, got {sk_rot:?}"
        );
        let idle = document
            .animations()
            .find(|a| a.name() == Some("IdleLookAround"))
            .expect("IdleLookAround");
        let jaw_quat = bone_rotation_at_clip_time(
            &document,
            &buffers,
            idle,
            "Bufomorph_JawUpper",
            0.0,
        )
        .expect("jaw rotation at t=0");
        for (actual, expected) in jaw_quat.iter().zip(BUFOMORPH_IDLE_JAW_ROTATION) {
            assert!(
                (*actual - expected).abs() < 2e-4,
                "IdleLookAround jaw should use clip-rig closed pose at t=0, got {jaw_quat:?}"
            );
        }
        let breathe = document
            .animations()
            .find(|a| a.name() == Some("IdleBreathe"))
            .expect("IdleBreathe");
        let breathe_jaw = bone_rotation_at_clip_time(
            &document,
            &buffers,
            breathe,
            "Bufomorph_JawUpper",
            0.0,
        )
        .expect("IdleBreathe jaw");
        assert!(
            jaw_quat.iter().zip(breathe_jaw).all(|(a, b)| (*a - b).abs() < 1e-5),
            "IdleLookAround and IdleBreathe share the same clip-rig jaw rest"
        );
    }

    #[test]
    #[cfg(feature = "dev")]
    fn bufomorph_bite_attack_preserves_authored_body_motion() {
        use std::path::PathBuf;

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/units/bufomorph.glb");
        if !path.is_file() {
            return;
        }
        let (document, buffers, _) = gltf::import(&path).expect("bufomorph glb");
        let bite = document
            .animations()
            .find(|a| a.name() == Some("BiteAttack"))
            .expect("BiteAttack");
        let body_trans = bone_translation_at_clip_time(
            &document,
            &buffers,
            bite,
            "Bufomorph_",
            0.35,
        )
        .expect("Bufomorph_ translation mid-clip");
        let magnitude = (body_trans[0].powi(2) + body_trans[1].powi(2) + body_trans[2].powi(2)).sqrt();
        assert!(
            magnitude > 0.15 && magnitude < 2.0,
            "BiteAttack hip motion must stay in metres (pack ~0.27 m), not Blender-merge 100x; got {body_trans:?}, |v|={magnitude}"
        );
    }

    #[test]
    #[cfg(feature = "dev")]
    fn bufomorph_pack_asset_includes_idle_breathe_clip() {
        use std::path::PathBuf;

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/units/bufomorph.glb");
        if !path.is_file() {
            return;
        }
        let (document, buffers, _) = gltf::import(&path).expect("bufomorph glb");
        let names: Vec<String> = document
            .animations()
            .map(|a| a.name().unwrap_or_default().to_string())
            .collect();
        assert!(
            names.iter().any(|name| name == "IdleBreathe"),
            "expected IdleBreathe in bufomorph glb, got: {names:?}"
        );

        let manifest_path = path.with_extension("export_manifest.json");
        if manifest_path.is_file() {
            let manifest_text = std::fs::read_to_string(&manifest_path).expect("export manifest");
            assert!(
                manifest_text.contains("\"complete\": true"),
                "bufomorph export manifest must report complete=true"
            );
            assert!(
                manifest_text.contains("\"pipeline\": \"fbx2gltf_gltf_merge\""),
                "bufomorph must be exported via FBX2glTF glTF merge, not Blender merge"
            );
            assert!(
                manifest_text.contains("\"have_count\": 21"),
                "bufomorph export manifest must list 21 clips"
            );
        }

        let idle = document
            .animations()
            .find(|a| a.name() == Some("IdleBreathe"))
            .expect("IdleBreathe");
        let (clip_start, clip_end) =
            animation_clip_time_range(&buffers, &idle).expect("IdleBreathe key times");
        assert!(
            clip_start < 0.05,
            "IdleBreathe should start at t=0 after export rebase, got start={clip_start}s"
        );
        assert!(
            clip_end > 1.0 && clip_end < 6.0,
            "IdleBreathe should be a short idle (~2s), got duration to {clip_end}s"
        );

        if let Some(tongue) = document
            .nodes()
            .find(|n| n.name() == Some("Bufomorph_Tongue1"))
        {
            let [sx, sy, sz] = tongue.transform().decomposed().2;
            assert!(
                sx > 0.5 && sy > 0.5 && sz > 0.5,
                "Tongue1 scale must not be collapsed by import workarounds, got [{sx}, {sy}, {sz}]"
            );
        }

        let body_index = document
            .nodes()
            .find(|n| n.name() == Some("Bufomorph_"))
            .map(|n| n.index());
        if let Some(body_index) = body_index {
            let body_rotated_in_idle = idle.channels().any(|c| {
                c.target().node().index() == body_index
                    && c.target().property() == gltf::animation::Property::Rotation
            });
            assert!(
                body_rotated_in_idle,
                "IdleBreathe should retain Bufomorph_ rotation channels (no locomotion strip hack)"
            );
        }

        let jaw_end = idle_jaw_track_end_seconds(&document, &buffers, idle)
            .expect("IdleBreathe Bufomorph_JawUpper rotation");
        assert!(
            jaw_end >= clip_end - 0.2,
            "IdleBreathe jaw track ends at {jaw_end}s before clip {clip_end}s; re-export from pack"
        );
    }

    fn animation_clip_time_range(
        buffers: &[gltf::buffer::Data],
        animation: &gltf::Animation,
    ) -> Option<(f32, f32)> {
        let mut start = f32::INFINITY;
        let mut end = f32::NEG_INFINITY;
        for channel in animation.channels() {
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()].0));
            let inputs = reader.read_inputs()?;
            for t in inputs {
                start = start.min(t);
                end = end.max(t);
            }
        }
        if start.is_finite() && end.is_finite() {
            Some((start, end))
        } else {
            None
        }
    }

    fn bone_rotation_at_clip_time(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        animation: gltf::Animation,
        bone_name: &str,
        time_s: f32,
    ) -> Option<[f32; 4]> {
        let bone = document.nodes().find(|n| n.name() == Some(bone_name))?;
        let bone_index = bone.index();
        let channel = animation.channels().find(|c| {
            c.target().node().index() == bone_index
                && c.target().property() == gltf::animation::Property::Rotation
        })?;
        sample_vec4_channel_at_time(buffers, channel, time_s)
    }

    fn bone_translation_at_clip_time(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        animation: gltf::Animation,
        bone_name: &str,
        time_s: f32,
    ) -> Option<[f32; 3]> {
        let bone = document.nodes().find(|n| n.name() == Some(bone_name))?;
        let bone_index = bone.index();
        let channel = animation.channels().find(|c| {
            c.target().node().index() == bone_index
                && c.target().property() == gltf::animation::Property::Translation
        })?;
        sample_vec3_channel_at_time(buffers, channel, time_s)
    }

    fn sample_vec4_channel_at_time(
        buffers: &[gltf::buffer::Data],
        channel: gltf::animation::Channel,
        time_s: f32,
    ) -> Option<[f32; 4]> {
        use gltf::animation::util::ReadOutputs;
        use gltf::animation::util::Rotations;
        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()].0));
        let inputs: Vec<f32> = reader.read_inputs()?.collect();
        let outputs = reader.read_outputs()?;
        let values: Vec<[f32; 4]> = match outputs {
            ReadOutputs::Rotations(Rotations::F32(iter)) => iter.collect(),
            _ => return None,
        };
        let idx = nearest_keyframe_index(&inputs, time_s)?;
        values.get(idx).copied()
    }

    fn sample_vec3_channel_at_time(
        buffers: &[gltf::buffer::Data],
        channel: gltf::animation::Channel,
        time_s: f32,
    ) -> Option<[f32; 3]> {
        use gltf::animation::util::ReadOutputs;
        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()].0));
        let inputs: Vec<f32> = reader.read_inputs()?.collect();
        let outputs = reader.read_outputs()?;
        let values: Vec<[f32; 3]> = match outputs {
            ReadOutputs::Translations(iter) => iter.collect(),
            _ => return None,
        };
        let idx = nearest_keyframe_index(&inputs, time_s)?;
        values.get(idx).copied()
    }

    fn nearest_keyframe_index(inputs: &[f32], time_s: f32) -> Option<usize> {
        inputs
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (*a - time_s)
                    .abs()
                    .partial_cmp(&(*b - time_s).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    fn idle_jaw_track_end_seconds(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        animation: gltf::Animation,
    ) -> Option<f32> {
        let jaw = document
            .nodes()
            .find(|n| n.name() == Some("Bufomorph_JawUpper"))?;
        let jaw_index = jaw.index();
        let channel = animation.channels().find(|c| {
            c.target().node().index() == jaw_index
                && c.target().property() == gltf::animation::Property::Rotation
        })?;
        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()].0));
        let inputs = reader.read_inputs()?;
        inputs.last()
    }

    #[test]
    fn missing_profile_is_error() {
        let definition = sample_definition(Some("missing"));
        let report = validate_definition_animation_assets(
            &definition,
            None,
            AnimationGltfValidationState::Ready,
            None,
            None,
        );
        assert!(report.has_error());
    }

    #[test]
    fn static_model_is_info() {
        let definition = sample_definition(None);
        let report = validate_definition_animation_assets(
            &definition,
            None,
            AnimationGltfValidationState::Ready,
            None,
            None,
        );
        assert!(report.issues.iter().any(|i| i.code == "static_model"));
    }

    #[test]
    fn pending_gltf_does_not_emit_missing_gltf_error() {
        let definition = sample_definition(Some("humanoid"));
        let report = validate_definition_animation_assets(
            &definition,
            Some(&crate::world::AnimationProfile::new(
                crate::world::AnimationProfileId::new("humanoid"),
                "Idle",
                None,
                None,
                1.0,
                true,
            )),
            AnimationGltfValidationState::Pending,
            None,
            None,
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.code == "missing_gltf")
        );
    }

    #[test]
    fn failed_gltf_load_emits_missing_gltf_error() {
        let definition = sample_definition(Some("humanoid"));
        let report = validate_definition_animation_assets(
            &definition,
            Some(&crate::world::AnimationProfile::new(
                crate::world::AnimationProfileId::new("humanoid"),
                "Idle",
                None,
                None,
                1.0,
                true,
            )),
            AnimationGltfValidationState::LoadFailed,
            None,
            None,
        );
        assert!(report.has_error());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "missing_gltf")
        );
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
