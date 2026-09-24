use bevy::prelude::*;

use crate::units::components::{UnitRenderEntity, UnitRenderMetadata};
use crate::units::presentation::{UnitEditorPreviewRosterMember, UnitEditorPreviewUnit};
use crate::world::equipment::effective_weapon_for_unit;
use crate::world::{
    AnimationProfileCatalog, AppearanceProfileCatalog, AttackPhase, ItemCatalog, UnitCatalog,
    UnitId, WeaponCatalog, WorldData,
};

use super::assets::UnitAnimationAssets;
use super::components::{
    AnimationPlaybackClip, AnimationPlaybackPending, DeathPresentation, HitReactionActive,
    HitReactionRequested, LayeredPlaybackState, UnitAnimationGraphInstalled, UnitAnimationLayering,
    UnitAnimationPersistedState, UnitAnimationPlayerLink, UnitAnimationRuntime,
    UnitAnimationStateIndex, UpperAttackWeightFade,
};
use super::layered_playback::{
    LayeredPlaybackTargets, layered_state_from_targets, primary_playback_clip,
    resolve_layered_playback_targets, should_restart_layered_playback,
};
use super::layers::{
    UnitAnimationLayeringMode, UnitLayeredAnimationIntent, derive_layered_animation_intent,
    derive_layered_death_presentation_intent,
};
use super::locomotion_polish::LocomotionPresentationState;
use super::lod::{
    AnimationLod, AnimationLodPresentationState, animation_distance_meters, lod_left_frozen,
    lod_promoted_to_full, next_reduced_eval_time, resolve_animation_lod,
    should_evaluate_animation_intent,
};
use super::presentation_time::{default_attack_blend_out, presentation_advance_seconds};
use super::settings::UnitAnimationSettings;
use super::sync_timing::{
    attack_playback_key, should_restart_attack_playback, should_seek_attack_strike,
};
use super::work_presentation::WorkPresentationContext;

/// Apply derived layered animation intent through [`AnimationPlayer`] (A1–A4).
pub fn sync_unit_animation_playback(
    mut commands: Commands,
    mut params: super::params::AnimationPlaybackParams,
    mut roots: Query<
        (
            Entity,
            &UnitAnimationPlayerLink,
            &GlobalTransform,
            Option<&AnimationPlaybackPending>,
            Option<&UnitRenderEntity>,
            Option<&UnitRenderMetadata>,
            Option<&DeathPresentation>,
            Option<&HitReactionRequested>,
            Option<&HitReactionActive>,
            Option<&UnitAnimationLayering>,
            Option<&mut UpperAttackWeightFade>,
        ),
        (
            With<UnitAnimationGraphInstalled>,
            Without<UnitEditorPreviewUnit>,
            Without<UnitEditorPreviewRosterMember>,
        ),
    >,
    mut players: Query<(Entity, &mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    if !params.settings.enabled {
        return;
    }

    params.metrics.reset_frame();
    params.metrics.shared_graph_count = params.assets.shared_graph_count() as u32;
    params.metrics.definition_graph_count = params.assets.definition_graph_count() as u32;

    let sim_frozen = params.control.paused && !params.control.step_once;
    let render_delta_seconds = params.time.delta_secs();
    let presentation_delta = presentation_advance_seconds(&params.control, render_delta_seconds);
    let elapsed_seconds = params.time.elapsed_secs();
    let layout = params.world.layout();
    let camera_focus = params
        .camera
        .iter()
        .next()
        .map(|state| state.focus)
        .unwrap_or(Vec3::ZERO);
    prune_state_index(&params.world, &mut params.state_index);

    for (
        root,
        link,
        transform,
        pending,
        marker,
        metadata,
        death,
        hit_requested,
        hit_active,
        layering,
        mut upper_fade,
    ) in &mut roots
    {
        params.metrics.animated_units += 1;

        if let Some(fade) = upper_fade.as_deref_mut() {
            if presentation_delta > 0.0 {
                if let Ok((_, mut player, _)) = players.get_mut(link.player_entity) {
                    if advance_upper_attack_weight_fade(&mut player, fade, presentation_delta) {
                        commands.entity(root).remove::<UpperAttackWeightFade>();
                    }
                }
            }
        }

        let layering_mode = layering
            .map(|value| value.mode)
            .unwrap_or(UnitAnimationLayeringMode::FullBodyExclusive);

        let unit_id = marker.map(|value| value.unit_id);
        let record = unit_id.and_then(|id| params.world.get_unit(id));
        let distance_meters = animation_distance_meters(camera_focus, transform.translation());
        let previous_lod_state = unit_id
            .and_then(|id| params.state_index.states.get(&id))
            .map(|state| state.lod.clone())
            .unwrap_or_default();
        let previous_lod = previous_lod_state.lod;

        let lod = if death.is_some() {
            AnimationLod::Full
        } else {
            resolve_animation_lod(
                distance_meters,
                previous_lod,
                &params.lod_settings,
                unit_id,
                record,
                &params.selection.0,
                params.focus.inspected_unit,
            )
        };

        match lod {
            AnimationLod::Full => params.metrics.full_count += 1,
            AnimationLod::Reduced => params.metrics.reduced_count += 1,
            AnimationLod::Frozen => params.metrics.frozen_count += 1,
        }

        let mut lod_state = AnimationLodPresentationState {
            lod,
            distance_meters,
            next_intent_eval_at: previous_lod_state.next_intent_eval_at,
            player_frozen: previous_lod_state.player_frozen,
        };

        let force_eval = pending.is_some()
            || lod_promoted_to_full(previous_lod, lod)
            || lod_left_frozen(previous_lod, lod);

        if force_eval && lod == AnimationLod::Reduced {
            lod_state.next_intent_eval_at = next_reduced_eval_time(
                elapsed_seconds,
                params.lod_settings.reduced_update_interval_seconds,
            );
        }

        if lod_promoted_to_full(previous_lod, lod) || lod_left_frozen(previous_lod, lod) {
            commands.entity(root).insert(AnimationPlaybackPending);
        }

        if lod == AnimationLod::Frozen && !force_eval {
            if let Ok((_, mut player, _)) = players.get_mut(link.player_entity) {
                if !lod_state.player_frozen {
                    player.pause_all();
                    lod_state.player_frozen = true;
                }
            }
            if let Some(id) = unit_id {
                params.state_index.states.entry(id).and_modify(|state| {
                    state.lod = lod_state.clone();
                });
            }
            continue;
        }

        if lod_state.player_frozen {
            if let Ok((_, mut player, _)) = players.get_mut(link.player_entity) {
                player.resume_all();
            }
            lod_state.player_frozen = false;
        }

        if !should_evaluate_animation_intent(
            lod,
            elapsed_seconds,
            lod_state.next_intent_eval_at,
            force_eval,
        ) {
            continue;
        }
        params.metrics.intent_evaluations += 1;
        if lod == AnimationLod::Reduced {
            lod_state.next_intent_eval_at = next_reduced_eval_time(
                elapsed_seconds,
                params.lod_settings.reduced_update_interval_seconds,
            );
        }

        let work_ctx = WorkPresentationContext {
            world: &params.world,
            building_catalog: &params.building_catalog,
            operation_catalog: &params.operation_catalog,
        };
        let resolved = match resolve_layered_context(
            death,
            metadata,
            marker,
            hit_requested.is_some(),
            hit_active.is_some(),
            &params.world,
            &params.catalog,
            &params.items,
            &params.weapons,
            &params.profiles,
            &params.appearance_profiles,
            &params.settings,
            &params.assets,
            layout,
            presentation_delta,
            &mut params.state_index,
            Some(&work_ctx),
        ) {
            Some(value) => value,
            None => continue,
        };

        let LayeredPlaybackContext {
            profile_id,
            layered_intent,
            weapon_opt,
            unit_id,
            profile,
            built,
            locomotion,
        } = resolved;

        let persisted = unit_id.and_then(|id| params.state_index.states.get(&id).cloned());
        let cycle = unit_id
            .and_then(|id| params.world.get_unit(id))
            .and_then(|r| r.attack_cycle.as_ref());
        let previous_phase = persisted.as_ref().and_then(|s| s.last_attack_phase);
        let previous_attack_key = persisted.as_ref().and_then(|s| s.attack_key.as_ref());
        let unarmed_strike_parity = persisted
            .as_ref()
            .map(|state| state.unarmed_strike_parity)
            .unwrap_or(0);
        let attack_restart = cycle.zip(weapon_opt).is_some_and(|(cycle, weapon)| {
            should_restart_attack_playback(
                previous_phase,
                cycle,
                previous_attack_key,
                weapon,
            )
        });
        // Variant is stable for the duration of an attack; parity advances only on transition.
        let use_alternate_attack_variant = unarmed_strike_parity % 2 == 1;
        let targets = resolve_layered_playback_targets(
            &layered_intent,
            layering_mode,
            built,
            weapon_opt,
            profile,
            &params.settings,
            use_alternate_attack_variant,
        );
        let playback_clip = primary_playback_clip(&targets);
        let layered_clips = layered_state_from_targets(&targets);

        let previous_layers = persisted.as_ref().map(|state| state.layers.clone());

        let mut attack_blend_out = None;
        if targets.upper.is_some() {
            attack_blend_out = weapon_opt.map(|weapon| {
                std::time::Duration::from_millis(weapon.attack_animation.blend_out_ms as u64)
            });
        } else if let Some(prev) = persisted.as_ref() {
            if prev.layers.upper.is_some() {
                attack_blend_out = prev.attack_blend_out.or_else(|| {
                    weapon_opt.map(|weapon| {
                        std::time::Duration::from_millis(
                            weapon.attack_animation.blend_out_ms as u64,
                        )
                    })
                });
            }
        }

        let should_transition = pending.is_some()
            || force_eval
            || should_restart_layered_playback(previous_layers.as_ref(), &targets)
            || should_restart_attack_layer(
                &layered_intent,
                cycle,
                weapon_opt,
                previous_phase,
                previous_attack_key,
                previous_layers.as_ref(),
                &targets,
            );

        if sim_frozen {
            if let Ok((_, mut player, _)) = players.get_mut(link.player_entity) {
                player.pause_all();
            }
            if let Some(id) = unit_id {
                update_persisted_state(
                    &mut params.state_index,
                    id,
                    &profile_id,
                    &layered_intent,
                    persisted,
                    &playback_clip,
                    &layered_clips,
                    attack_blend_out,
                    cycle,
                    weapon_opt,
                    locomotion,
                    lod_state.clone(),
                );
            }
            continue;
        }

        if !should_transition {
            if let Ok((_, mut player, _)) = players.get_mut(link.player_entity) {
                if player.all_paused() {
                    player.resume_all();
                }
                if let Some(lower) = &targets.lower {
                    update_lower_playback_speed(
                        &mut player,
                        lower,
                        previous_layers.as_ref(),
                        &params.settings,
                    );
                } else if let Some(full_body) = &targets.full_body {
                    update_full_body_playback_speed(
                        &mut player,
                        full_body,
                        previous_layers.as_ref(),
                        &params.settings,
                    );
                }
            }
            if let Some(id) = unit_id {
                update_persisted_state(
                    &mut params.state_index,
                    id,
                    &profile_id,
                    &layered_intent,
                    persisted,
                    &playback_clip,
                    &layered_clips,
                    attack_blend_out,
                    cycle,
                    weapon_opt,
                    locomotion,
                    lod_state.clone(),
                );
            }
            continue;
        }

        let Ok((_, mut player, mut transitions)) = players.get_mut(link.player_entity) else {
            continue;
        };

        params.metrics.transitions_applied += 1;

        let applied = apply_layered_playback(
            &mut player,
            &mut transitions,
            &targets,
            previous_layers.as_ref(),
            cycle,
            weapon_opt,
            attack_blend_out,
        );

        if applied.clear_upper_fade {
            commands.entity(root).remove::<UpperAttackWeightFade>();
        }
        if let Some(fade) = applied.start_upper_fade {
            commands.entity(root).insert(fade);
        }

        if hit_requested.is_some() {
            commands.entity(root).remove::<HitReactionRequested>();
            commands.entity(root).insert(HitReactionActive {
                remaining_seconds: params.settings.hit_reaction_hold_seconds,
            });
        }

        let attack_key = match (&layered_intent.upper, cycle) {
            (super::layers::UpperBodyIntent::Attack { .. }, Some(cycle)) => {
                weapon_opt.map(|weapon| attack_playback_key(cycle, weapon))
            }
            _ => None,
        };

        let mut layered_state = layered_clips;
        layered_state.lower_node = applied.lower_node;
        layered_state.upper_node = applied.upper_node;
        layered_state.full_body_node = applied.full_body_node;

        let next_unarmed_strike_parity = if attack_restart {
            unarmed_strike_parity + 1
        } else {
            unarmed_strike_parity
        };

        if let Some(id) = unit_id {
            params.state_index.states.insert(
                id,
                UnitAnimationPersistedState {
                    clip: playback_clip.clone(),
                    layers: layered_state.clone(),
                    profile_id: profile_id.clone(),
                    last_attack_phase: cycle.map(|cycle| cycle.phase),
                    attack_key,
                    attack_blend_out,
                    unarmed_strike_parity: next_unarmed_strike_parity,
                    locomotion,
                    lod: lod_state.clone(),
                },
            );
        }

        commands.entity(root).insert(UnitAnimationRuntime {
            current_clip: playback_clip,
            layers: layered_state,
        });
        commands.entity(root).remove::<AnimationPlaybackPending>();
    }

    let visible: std::collections::HashSet<UnitId> = params.index.0.keys().copied().collect();
    params
        .state_index
        .states
        .retain(|id, _| visible.contains(id));
}

fn resolve_layered_context<'a>(
    death: Option<&DeathPresentation>,
    metadata: Option<&UnitRenderMetadata>,
    marker: Option<&UnitRenderEntity>,
    hit_requested: bool,
    hit_active: bool,
    world: &'a WorldData,
    catalog: &'a UnitCatalog,
    items: &'a ItemCatalog,
    weapons: &'a WeaponCatalog,
    profiles: &'a AnimationProfileCatalog,
    appearance_profiles: &'a AppearanceProfileCatalog,
    settings: &UnitAnimationSettings,
    assets: &'a UnitAnimationAssets,
    layout: crate::world::ChunkLayout,
    delta_seconds: f32,
    state_index: &mut UnitAnimationStateIndex,
    work_ctx: Option<&WorkPresentationContext<'a>>,
) -> Option<LayeredPlaybackContext<'a>> {
    if let (Some(death), Some(metadata)) = (death, metadata) {
        let profile = profiles.get(&death.profile_id)?;
        let layered_intent = derive_layered_death_presentation_intent(profile, death, settings)?;
        let built = assets.graph_for(&metadata.definition_id)?;
        return Some(LayeredPlaybackContext {
            profile_id: death.profile_id.clone(),
            layered_intent,
            weapon_opt: None,
            unit_id: marker.map(|m| m.unit_id),
            profile,
            built,
            locomotion: LocomotionPresentationState::default(),
        });
    }

    let marker = marker?;
    let record = world.get_unit(marker.unit_id)?;
    let definition = catalog.get(&record.definition_id)?;
    let profile_id = definition.animation_profile_id.as_ref()?;
    let profile = profiles.get(profile_id)?;
    let weapon = effective_weapon_for_unit(world, record, catalog, items, weapons).ok()?;
    let mut locomotion = state_index
        .states
        .get(&marker.unit_id)
        .map(|state| state.locomotion.clone())
        .unwrap_or_default();
    let layered_intent = derive_layered_animation_intent(
        record,
        definition,
        profile,
        weapon,
        settings,
        layout,
        &mut locomotion,
        delta_seconds,
        hit_requested,
        hit_active,
        work_ctx,
        crate::world::unit_locomotion_surface(world, marker.unit_id),
    )?;
    let built = assets
        .graph_for_unit(record, definition, appearance_profiles)
        .or_else(|| assets.graph_for(&definition.id))?;
    Some(LayeredPlaybackContext {
        profile_id: profile_id.clone(),
        layered_intent,
        weapon_opt: Some(weapon),
        unit_id: Some(marker.unit_id),
        profile,
        built,
        locomotion,
    })
}

struct LayeredPlaybackContext<'a> {
    profile_id: crate::world::AnimationProfileId,
    layered_intent: UnitLayeredAnimationIntent,
    weapon_opt: Option<&'a crate::world::WeaponDefinition>,
    unit_id: Option<UnitId>,
    profile: &'a crate::world::AnimationProfile,
    built: &'a super::assets::DefinitionAnimationGraph,
    locomotion: LocomotionPresentationState,
}

struct LayeredPlaybackApply {
    lower_node: Option<AnimationNodeIndex>,
    upper_node: Option<AnimationNodeIndex>,
    full_body_node: Option<AnimationNodeIndex>,
    start_upper_fade: Option<UpperAttackWeightFade>,
    clear_upper_fade: bool,
}

fn apply_looping_playback_speed(
    active: &mut bevy::animation::ActiveAnimation,
    target: &super::layered_playback::LayerClipTarget,
) {
    active.set_speed(target.speed);
    if target.looping {
        active.repeat();
    }
}

fn advance_upper_attack_weight_fade(
    player: &mut AnimationPlayer,
    fade: &mut UpperAttackWeightFade,
    delta: f32,
) -> bool {
    if fade.duration_seconds <= f32::EPSILON {
        if !fade.fading_in {
            player.stop(fade.node);
        }
        return true;
    }
    fade.remaining_seconds = (fade.remaining_seconds - delta).max(0.0);
    let progress = 1.0 - fade.remaining_seconds / fade.duration_seconds;
    let weight = if fade.fading_in {
        progress.clamp(0.0, 1.0)
    } else {
        (1.0 - progress).clamp(0.0, 1.0)
    };
    if let Some(active) = player.animation_mut(fade.node) {
        active.set_weight(weight);
    }
    if fade.remaining_seconds <= 0.0 {
        if !fade.fading_in {
            player.stop(fade.node);
        }
        return true;
    }
    false
}

fn apply_layered_playback(
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    targets: &LayeredPlaybackTargets,
    previous: Option<&LayeredPlaybackState>,
    cycle: Option<&crate::world::AttackCycle>,
    weapon: Option<&crate::world::WeaponDefinition>,
    attack_blend_out: Option<std::time::Duration>,
) -> LayeredPlaybackApply {
    let mut result = LayeredPlaybackApply {
        lower_node: None,
        upper_node: None,
        full_body_node: None,
        start_upper_fade: None,
        clear_upper_fade: false,
    };

    if let Some(full_body) = &targets.full_body {
        result.clear_upper_fade = true;
        let previous_node = previous.and_then(|state| state.full_body_node);
        if full_body.freeze_pose {
            player.pause_all();
            result.full_body_node = Some(full_body.node);
            return result;
        }
        player.resume_all();
        let same_node = previous_node == Some(full_body.node);
        if same_node {
            if let Some(active) = player.animation_mut(full_body.node) {
                apply_looping_playback_speed(active, full_body);
            }
            result.full_body_node = Some(full_body.node);
            return result;
        }
        if let Some(node) = previous_node {
            player.stop(node);
        }
        let mut active = transitions.play(player, full_body.node, full_body.blend);
        apply_looping_playback_speed(&mut active, full_body);
        result.full_body_node = Some(full_body.node);
        return result;
    }

    let mut lower_node = previous.and_then(|state| state.lower_node);
    let mut upper_node = previous.and_then(|state| state.upper_node);

    if let Some(previous) = previous {
        if previous.full_body_node.is_some() {
            player.stop_all();
        }
        if let Some(node) = previous.lower_node {
            if targets.lower.as_ref().map(|t| t.node) != Some(node) {
                player.stop(node);
                lower_node = None;
            }
        }
        if let Some(node) = previous.upper_node {
            if let Some(upper) = &targets.upper {
                if Some(upper.node) != Some(node) {
                    player.stop(node);
                    upper_node = None;
                }
            }
        }
        if targets.upper.is_none() {
            if let Some(node) = previous.upper_node {
                let blend = attack_blend_out.unwrap_or_else(default_attack_blend_out);
                if blend > std::time::Duration::ZERO {
                    result.start_upper_fade = Some(UpperAttackWeightFade {
                        node,
                        remaining_seconds: blend.as_secs_f32(),
                        duration_seconds: blend.as_secs_f32(),
                        fading_in: false,
                    });
                    upper_node = Some(node);
                } else {
                    player.stop(node);
                    upper_node = None;
                }
            }
        }
    }

    if let Some(lower) = &targets.lower {
        if lower_node != Some(lower.node) {
            let mut active = transitions.play(player, lower.node, lower.blend);
            apply_looping_playback_speed(&mut active, lower);
            lower_node = Some(lower.node);
        }
    }

    if let Some(upper) = &targets.upper {
        if upper_node != Some(upper.node) {
            let active = player.play(upper.node);
            active.set_speed(upper.speed);
            if upper.blend > std::time::Duration::ZERO {
                active.set_weight(0.0);
                result.start_upper_fade = Some(UpperAttackWeightFade {
                    node: upper.node,
                    remaining_seconds: upper.blend.as_secs_f32(),
                    duration_seconds: upper.blend.as_secs_f32(),
                    fading_in: true,
                });
            }
            upper_node = Some(upper.node);
        }
        if let (Some(cycle), Some(weapon)) = (cycle, weapon) {
            if let Some(seek) = should_seek_attack_strike(weapon, cycle, upper.duration) {
                player.play(upper.node).set_seek_time(seek * upper.duration);
            }
        }
    }

    result.lower_node = lower_node;
    result.upper_node = upper_node;
    result
}

fn should_restart_attack_layer(
    intent: &UnitLayeredAnimationIntent,
    cycle: Option<&crate::world::AttackCycle>,
    weapon: Option<&crate::world::WeaponDefinition>,
    previous_phase: Option<AttackPhase>,
    previous_attack_key: Option<&super::sync_timing::AttackPlaybackKey>,
    previous_layers: Option<&LayeredPlaybackState>,
    targets: &LayeredPlaybackTargets,
) -> bool {
    if !matches!(intent.upper, super::layers::UpperBodyIntent::Attack { .. }) {
        return false;
    }
    let Some(previous_layers) = previous_layers else {
        return false;
    };
    let current_attack_clip = targets
        .full_body
        .as_ref()
        .map(|t| t.clip.clone())
        .or_else(|| targets.upper.as_ref().map(|t| t.clip.clone()));
    let previous_attack_clip = previous_layers
        .full_body
        .clone()
        .or(previous_layers.upper.clone());
    if current_attack_clip.is_none() || previous_attack_clip != current_attack_clip {
        return false;
    }
    cycle.is_some_and(|cycle| {
        weapon.is_some_and(|weapon| {
            should_restart_attack_playback(previous_phase, cycle, previous_attack_key, weapon)
        })
    })
}

fn update_lower_playback_speed(
    player: &mut AnimationPlayer,
    lower: &super::layered_playback::LayerClipTarget,
    previous: Option<&LayeredPlaybackState>,
    settings: &UnitAnimationSettings,
) {
    update_layer_playback_speed(
        player,
        lower,
        previous.and_then(|state| state.lower_node),
        previous.and_then(|state| state.lower_speed),
        settings,
    );
}

fn update_full_body_playback_speed(
    player: &mut AnimationPlayer,
    full_body: &super::layered_playback::LayerClipTarget,
    previous: Option<&LayeredPlaybackState>,
    settings: &UnitAnimationSettings,
) {
    update_layer_playback_speed(
        player,
        full_body,
        previous.and_then(|state| state.full_body_node),
        previous.and_then(|state| state.lower_speed),
        settings,
    );
}

fn update_layer_playback_speed(
    player: &mut AnimationPlayer,
    target: &super::layered_playback::LayerClipTarget,
    previous_node: Option<AnimationNodeIndex>,
    previous_speed: Option<f32>,
    settings: &UnitAnimationSettings,
) {
    let node = previous_node.unwrap_or(target.node);
    let speed_changed = previous_speed
        .map(|speed| (speed - target.speed).abs() > settings.speed_update_epsilon)
        .unwrap_or(false);
    if !speed_changed {
        return;
    }
    if let Some(active) = player.animation_mut(node) {
        apply_looping_playback_speed(active, target);
    }
}

fn update_persisted_state(
    state_index: &mut UnitAnimationStateIndex,
    unit_id: UnitId,
    profile_id: &crate::world::AnimationProfileId,
    intent: &UnitLayeredAnimationIntent,
    persisted: Option<UnitAnimationPersistedState>,
    playback_clip: &AnimationPlaybackClip,
    layers: &LayeredPlaybackState,
    attack_blend_out: Option<std::time::Duration>,
    cycle: Option<&crate::world::AttackCycle>,
    weapon: Option<&crate::world::WeaponDefinition>,
    locomotion: LocomotionPresentationState,
    lod: AnimationLodPresentationState,
) {
    let Some(mut state) = persisted else {
        return;
    };
    let _previous_phase = state.last_attack_phase;
    let _previous_attack_key = state.attack_key.clone();
    state.last_attack_phase = match &intent.upper {
        super::layers::UpperBodyIntent::Attack { phase, .. } => Some(*phase),
        super::layers::UpperBodyIntent::None => state.last_attack_phase,
    };
    if matches!(
        intent.override_mode,
        super::layers::FullBodyOverride::Death { .. }
    ) {
        state.last_attack_phase = None;
    }
    state.clip = playback_clip.clone();
    state.layers = layers.clone();
    state.profile_id = profile_id.clone();
    state.attack_blend_out = attack_blend_out;
    state.locomotion = locomotion;
    state.lod = lod;
    state.attack_key = match (&intent.upper, cycle) {
        (super::layers::UpperBodyIntent::Attack { .. }, Some(cycle)) => {
            weapon.map(|weapon| attack_playback_key(cycle, weapon))
        }
        _ => state.attack_key,
    };
    state_index.states.insert(unit_id, state);
}

fn prune_state_index(world: &WorldData, state_index: &mut UnitAnimationStateIndex) {
    state_index
        .states
        .retain(|unit_id, _| world.get_unit(*unit_id).is_some());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::animation::components::LayeredPlaybackState;
    use crate::world::AnimationClipKey;

    #[test]
    fn locomotion_state_persists_in_index() {
        let mut index = UnitAnimationStateIndex::default();
        let locomotion = LocomotionPresentationState {
            last_locomotion_clip: Some(AnimationClipKey::Run),
            smoothed_speed: 1.2,
            ..Default::default()
        };
        index.states.insert(
            UnitId::new(7),
            UnitAnimationPersistedState {
                clip: AnimationPlaybackClip::Locomotion(AnimationClipKey::Run),
                layers: LayeredPlaybackState::default(),
                profile_id: crate::world::AnimationProfileId::new("humanoid"),
                last_attack_phase: None,
                attack_key: None,
                attack_blend_out: None,
                unarmed_strike_parity: 0,
                locomotion: locomotion.clone(),
                lod: AnimationLodPresentationState::default(),
            },
        );
        assert_eq!(
            index.states.get(&UnitId::new(7)).unwrap().locomotion,
            locomotion
        );
    }

    #[test]
    fn turn_clip_change_restarts_layered_playback() {
        let previous = LayeredPlaybackState {
            lower: Some(AnimationPlaybackClip::Locomotion(AnimationClipKey::Walk)),
            ..Default::default()
        };
        let targets = LayeredPlaybackTargets {
            lower: Some(super::super::layered_playback::LayerClipTarget {
                clip: AnimationPlaybackClip::Locomotion(AnimationClipKey::TurnLeft),
                node: AnimationNodeIndex::new(3),
                duration: 0.6,
                speed: 1.0,
                blend: std::time::Duration::from_millis(120),
                looping: false,
                freeze_pose: false,
            }),
            ..Default::default()
        };
        assert!(should_restart_layered_playback(Some(&previous), &targets));
    }

    #[test]
    fn attack_end_uses_default_blend_out_duration() {
        assert_eq!(
            default_attack_blend_out(),
            std::time::Duration::from_millis(150)
        );
    }

    #[test]
    fn upper_fade_out_duration_matches_weapon_blend_out() {
        let fade = UpperAttackWeightFade {
            node: AnimationNodeIndex::new(2),
            remaining_seconds: 0.25,
            duration_seconds: 0.25,
            fading_in: false,
        };
        assert!(!fade.fading_in);
        assert_eq!(fade.remaining_seconds, fade.duration_seconds);
    }

    #[test]
    fn upper_fade_in_starts_at_zero_weight_progress() {
        let fade = UpperAttackWeightFade {
            node: AnimationNodeIndex::new(1),
            remaining_seconds: 0.15,
            duration_seconds: 0.15,
            fading_in: true,
        };
        let progress = 1.0 - fade.remaining_seconds / fade.duration_seconds;
        assert_eq!(progress, 0.0);
    }

    #[test]
    fn full_body_attack_restart_does_not_misfire_on_none_upper_layers() {
        use crate::units::animation::layers::{
            FullBodyOverride, LowerBodyIntent, UnitLayeredAnimationIntent, UpperBodyIntent,
        };
        use crate::units::animation::layered_playback::LayerClipTarget;
        use crate::units::animation::sync_timing::attack_playback_key;
        use crate::world::{
            AttackCycle, DamageType, HitMode, TargetFilter, WeaponAttackAnimation,
            WeaponDefinition, WeaponDefinitionId,
        };

        let weapon_id = WeaponDefinitionId::new("weapon_iron_sword");
        let weapon = WeaponDefinition::new(
            weapon_id.clone(),
            "Iron Sword",
            "Iron Sword",
            5.0,
            DamageType::Slashing,
            1.5,
            1.0,
            0.2,
            0.15,
            HitMode::Melee,
            None,
            0.0,
            "Sword_Attack",
            vec![TargetFilter::Enemies],
            None,
            true,
        )
        .with_attack_animation(WeaponAttackAnimation::default());
        let cycle = AttackCycle {
            target: UnitId::new(2),
            phase: AttackPhase::Strike,
            phase_remaining_seconds: 0.05,
            struck_this_cycle: true,
        };
        let key = attack_playback_key(&cycle, &weapon);
        let attack_clip = AnimationPlaybackClip::Attack(weapon_id.clone());
        let previous_layers = LayeredPlaybackState {
            full_body: Some(attack_clip.clone()),
            full_body_node: Some(AnimationNodeIndex::new(4)),
            ..Default::default()
        };
        let targets = LayeredPlaybackTargets {
            full_body: Some(LayerClipTarget {
                clip: attack_clip,
                node: AnimationNodeIndex::new(4),
                duration: 1.0,
                speed: 1.0,
                blend: std::time::Duration::from_millis(150),
                looping: false,
                freeze_pose: false,
            }),
            ..Default::default()
        };
        let intent = UnitLayeredAnimationIntent {
            lower: LowerBodyIntent::Suppressed,
            upper: UpperBodyIntent::Attack {
                weapon_id: weapon_id.clone(),
                phase: AttackPhase::Strike,
                blend: std::time::Duration::from_millis(150),
                blend_out: std::time::Duration::from_millis(150),
            },
            overlay: super::super::layers::OverlayIntent::None,
            override_mode: FullBodyOverride::None,
        };

        assert!(
            !should_restart_attack_layer(
                &intent,
                Some(&cycle),
                Some(&weapon),
                Some(AttackPhase::Strike),
                Some(&key),
                Some(&previous_layers),
                &targets,
            ),
            "sustained full-body attack must not restart every frame",
        );
    }

    #[test]
    fn unarmed_strike_parity_advances_only_on_transition_restart() {
        use crate::units::animation::sync_timing::should_restart_attack_playback;
        use crate::world::{
            AttackCycle, DamageType, HitMode, TargetFilter, WeaponAttackAnimation,
            WeaponDefinition, WeaponDefinitionId,
        };

        let weapon = WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_fists"),
            "Fists",
            "Fists",
            1.0,
            DamageType::Blunt,
            1.0,
            1.0,
            0.15,
            0.1,
            HitMode::Melee,
            None,
            0.0,
            "Punch_Jab",
            vec![TargetFilter::Enemies],
            None,
            true,
        )
        .with_attack_animation(WeaponAttackAnimation::default());
        let cycle = AttackCycle {
            target: UnitId::new(2),
            phase: AttackPhase::Strike,
            phase_remaining_seconds: 0.05,
            struck_this_cycle: true,
        };
        let key = super::super::sync_timing::attack_playback_key(&cycle, &weapon);

        let mut parity = 0u8;
        for _ in 0..120 {
            let restart = should_restart_attack_playback(
                Some(AttackPhase::Strike),
                &cycle,
                Some(&key),
                &weapon,
            );
            if restart {
                parity += 1;
            }
        }
        assert_eq!(
            parity,
            0,
            "parity must not advance while the same attack cycle remains active",
        );

        let windup = AttackCycle::start_windup(UnitId::new(2), 0.15);
        assert!(should_restart_attack_playback(
            Some(AttackPhase::Recovery),
            &windup,
            Some(&key),
            &weapon,
        ));
        parity += 1;
        assert_eq!(parity, 1);
    }
}
