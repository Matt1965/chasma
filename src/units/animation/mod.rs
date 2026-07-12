//! Unit animation presentation (A1).

mod assets;
mod components;
mod death_presentation;
mod discovery;
mod hit_reaction;
mod intent;
mod layered_playback;
mod layers;
mod locomotion_polish;
mod lod;
mod off_screen_death;
mod params;
mod playback;
mod plugin;
mod presentation_time;
mod settings;
mod skeleton;
mod stress;
mod sync;
mod sync_timing;
mod validation;

#[allow(unused_imports)]
pub use assets::{
    AnimationGraphShareKey, DefinitionAnimationGraph, UnitAnimationAssets,
    build_unit_animation_graphs, install_animation_graph_on_player, preload_unit_animation_gltfs,
};
#[allow(unused_imports)]
pub use components::{
    AnimationPlaybackClip, AnimationPlaybackPending, AnimationProfileHandle, DeathPresentation,
    HitReactionActive, HitReactionRequested, LayeredPlaybackState, PendingAnimationLink,
    UnitAnimationGraphInstalled, UnitAnimationLayering, UnitAnimationPersistedState,
    UnitAnimationPlayerLink, UnitAnimationRuntime, UnitAnimationStateIndex, UpperAttackWeightFade,
};
#[allow(unused_imports)]
pub use death_presentation::{begin_death_presentations, tick_death_presentations};
#[allow(unused_imports)]
pub use discovery::{
    cleanup_animation_links_for_removed_roots, discover_unit_animation_players,
    heal_stale_animation_player_links, retry_pending_animation_links,
};
#[allow(unused_imports)]
pub use hit_reaction::{UnitHpPresentationCache, detect_unit_hit_reactions, tick_hit_reactions};
#[allow(unused_imports)]
pub use intent::{
    UnitAnimationIntent, attack_intent_speed, derive_unit_animation_intent,
    resolve_attack_clip_name,
};
#[allow(unused_imports)]
pub use layers::{
    UnitAnimationLayeringMode, UnitLayeredAnimationIntent, derive_layered_animation_intent,
    derive_layered_death_presentation_intent,
};
#[allow(unused_imports)]
pub use locomotion_polish::{
    LocomotionDebugSnapshot, LocomotionPresentationState, MODEL_FORWARD_AXIS,
    locomotion_debug_snapshot, locomotion_playback_speed, model_forward_xz, movement_heading_delta,
};
#[allow(unused_imports)]
pub use lod::{
    AnimationLod, AnimationLodPresentationState, AnimationLodSettings, AnimationPresentationFocus,
    AnimationPresentationMetrics, animation_distance_meters, lod_left_frozen, lod_promoted_to_full,
    next_reduced_eval_time, raw_animation_lod, resolve_animation_lod,
    should_evaluate_animation_intent,
};
#[allow(unused_imports)]
pub use playback::sync_unit_animation_playback;
#[allow(unused_imports)]
pub use plugin::{UnitAnimationPlugin, UnitAnimationSystems};
#[allow(unused_imports)]
pub use settings::{DOCUMENTED_RUN_SPEED_RATIO, UnitAnimationSettings};
#[allow(unused_imports)]
pub use skeleton::configure_unit_animation_layering;
#[allow(unused_imports)]
pub use sync_timing::{
    AttackPlaybackKey, attack_cycle_playback_seconds, attack_playback_speed,
    should_restart_attack_playback,
};
#[allow(unused_imports)]
pub use validation::{
    AnimationValidationIndex, DefinitionValidationReport, ValidationIssue, ValidationSeverity,
};
