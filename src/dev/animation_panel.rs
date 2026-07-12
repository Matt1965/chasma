//! Animation debug readout for Dev Mode Debug tab (A5/A6).

use bevy::prelude::*;

use crate::units::{
    AnimationPresentationMetrics, UnitAnimationAssets, UnitAnimationPlayerLink,
    UnitAnimationRuntime, UnitAnimationSettings, UnitAnimationStateIndex, UnitRenderEntity,
    ValidationSeverity, locomotion_debug_snapshot,
};
use crate::world::{AnimationProfileCatalog, UnitCatalog, WorldData};

use super::dev_mode::{DevModeState, DevTab};
use super::inspector::WorldInspectorState;
use super::panel::DevAnimationText;

/// Refresh animation debug text for the inspector-selected unit.
pub fn sync_dev_animation_panel(
    dev_state: Res<DevModeState>,
    inspector: Res<WorldInspectorState>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    profiles: Res<AnimationProfileCatalog>,
    settings: Res<UnitAnimationSettings>,
    state_index: Res<UnitAnimationStateIndex>,
    assets: Res<UnitAnimationAssets>,
    metrics: Res<AnimationPresentationMetrics>,
    markers: Query<(
        &UnitRenderEntity,
        &UnitAnimationPlayerLink,
        &UnitAnimationRuntime,
    )>,
    mut text: Query<(&mut Text, &mut Node), With<DevAnimationText>>,
) {
    let Ok((mut label, mut node)) = text.single_mut() else {
        return;
    };

    if !dev_state.enabled || dev_state.active_tab != DevTab::Debug {
        node.display = Display::None;
        **label = String::new();
        return;
    }
    node.display = Display::Flex;

    let aggregate = format!(
        "Anim aggregate: units={} full={} reduced={} frozen={} evals={} transitions={}\nshared_graphs={} definition_graphs={} missing_profiles={} missing_clips={}",
        metrics.animated_units,
        metrics.full_count,
        metrics.reduced_count,
        metrics.frozen_count,
        metrics.intent_evaluations,
        metrics.transitions_applied,
        metrics.shared_graph_count,
        metrics.definition_graph_count,
        assets.validation.missing_profile_count(),
        assets.validation.missing_clip_count(),
    );

    let Some(unit_id) = inspector.selected_unit else {
        **label = format!("{aggregate}\nAnimation: select a unit (Inspector tab, Alt+click)");
        return;
    };

    let Some(record) = world.get_unit(unit_id) else {
        **label = format!(
            "{aggregate}\nAnimation: unit {} not in WorldData",
            unit_id.raw()
        );
        return;
    };

    let definition = catalog.get(&record.definition_id);
    let profile_id = definition.and_then(|def| def.animation_profile_id.as_ref());
    let profile = profile_id.and_then(|id| profiles.get(id));
    let persisted = state_index.states.get(&unit_id);
    let locomotion = persisted
        .map(|state| state.locomotion.clone())
        .unwrap_or_default();
    let locomotion_debug =
        locomotion_debug_snapshot(record, world.layout(), &locomotion, &settings);

    let runtime_info = markers
        .iter()
        .find(|(marker, _, _)| marker.unit_id == unit_id);

    let graph_info = definition
        .and_then(|def| assets.graph_for(&def.id))
        .map(|built| {
            let missing = [
                (crate::world::AnimationClipKey::Idle, "Idle"),
                (crate::world::AnimationClipKey::Walk, "Walk"),
                (crate::world::AnimationClipKey::Run, "Run"),
                (crate::world::AnimationClipKey::TurnLeft, "TurnLeft"),
                (crate::world::AnimationClipKey::TurnRight, "TurnRight"),
            ]
            .iter()
            .filter(|(key, _)| !built.locomotion_nodes.contains_key(key))
            .map(|(_, label)| *label)
            .collect::<Vec<_>>();
            (
                built.profile_id.as_str().to_string(),
                built.share_key.gltf_asset_path.clone(),
                missing,
            )
        });

    let layer_lines = runtime_info.map(|(_, link, runtime)| {
        format!(
            "player={:?} layers lower={:?} upper={:?} full={:?}\nspeed={:?} blend_ms={:?}",
            link.player_entity,
            runtime.layers.lower,
            runtime.layers.upper,
            runtime.layers.full_body,
            runtime.layers.lower_speed,
            runtime.layers.lower_blend_ms,
        )
    });

    let lod_line = persisted
        .map(|state| {
            format!(
                "LOD={} dist={:.1}m next_eval={:.2}s frozen={}",
                state.lod.lod.label(),
                state.lod.distance_meters,
                state.lod.next_intent_eval_at,
                state.lod.player_frozen
            )
        })
        .unwrap_or_else(|| "LOD=unknown".into());

    let validation_line = assets
        .validation_for(&record.definition_id)
        .map(|report| {
            let errors = report
                .issues
                .iter()
                .filter(|i| i.severity == ValidationSeverity::Error)
                .count();
            let warnings = report
                .issues
                .iter()
                .filter(|i| i.severity == ValidationSeverity::Warning)
                .count();
            format!("validation errors={errors} warnings={warnings}")
        })
        .unwrap_or_else(|| "validation: n/a".into());

    let profile_line = profile
        .map(|p| {
            format!(
                "profile={} ref_speed={:.2} m/s",
                p.id.as_str(),
                p.locomotion_reference_speed_mps
            )
        })
        .unwrap_or_else(|| "profile: none (static model)".into());

    let clip_line = locomotion_debug
        .locomotion_clip
        .map(|clip| clip.label().to_string())
        .unwrap_or_else(|| "none".into());

    let graph_line = graph_info
        .map(|(id, path, missing)| {
            if missing.is_empty() {
                format!("graph profile={id} asset={path} missing=none")
            } else {
                format!(
                    "graph profile={id} asset={path} missing={}",
                    missing.join(", ")
                )
            }
        })
        .unwrap_or_else(|| "graph: not built".into());

    **label = format!(
        "{aggregate}\nAnimation unit={}\n{profile_line}\n{lod_line}\nclip={clip_line} speed={:.2} turn={} heading={:?} align={:.2}\n{graph_line}\n{validation_line}\n{}",
        unit_id.raw(),
        locomotion_debug.playback_speed,
        locomotion_debug.turn_active,
        locomotion_debug
            .heading_delta_degrees
            .map(|deg| format!("{deg:.1}°")),
        locomotion_debug.alignment_factor,
        layer_lines.unwrap_or_else(|| "runtime: no linked player".into()),
    );
}
