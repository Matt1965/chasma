use bevy::prelude::*;

use crate::units::sync::UnitRuntimeSystems;

use super::assets::{
    build_unit_animation_graphs, install_animation_graph_on_player, preload_unit_animation_gltfs,
};
use super::death_presentation::{begin_death_presentations, tick_death_presentations};
use super::discovery::{
    cleanup_animation_links_for_removed_roots, discover_unit_animation_players,
    heal_stale_animation_player_links, retry_pending_animation_links,
};
use super::hit_reaction::{UnitHpPresentationCache, detect_unit_hit_reactions, tick_hit_reactions};
use super::playback::sync_unit_animation_playback;
use super::settings::UnitAnimationSettings;
use super::skeleton::configure_unit_animation_layering;
use super::{
    AnimationLodSettings, AnimationPlaybackClip, AnimationPlaybackPending,
    AnimationPresentationFocus, AnimationPresentationMetrics, AnimationProfileHandle,
    DeathPresentation, HitReactionActive, HitReactionRequested, PendingAnimationLink,
    UnitAnimationGraphInstalled, UnitAnimationLayering, UnitAnimationPlayerLink,
    UnitAnimationRuntime, UnitAnimationStateIndex, UpperAttackWeightFade,
};

/// Systems that drive derived unit animation presentation (A1).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct UnitAnimationSystems;

pub struct UnitAnimationPlugin;

impl Plugin for UnitAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<UnitAnimationSettings>()
            .register_type::<UnitAnimationPlayerLink>()
            .register_type::<PendingAnimationLink>()
            .register_type::<AnimationPlaybackClip>()
            .register_type::<UnitAnimationRuntime>()
            .register_type::<UnitAnimationGraphInstalled>()
            .register_type::<AnimationPlaybackPending>()
            .register_type::<AnimationProfileHandle>()
            .register_type::<DeathPresentation>()
            .register_type::<HitReactionRequested>()
            .register_type::<HitReactionActive>()
            .register_type::<UnitAnimationLayering>()
            .register_type::<UpperAttackWeightFade>()
            .register_type::<AnimationLodSettings>()
            .init_resource::<UnitAnimationSettings>()
            .init_resource::<AnimationLodSettings>()
            .init_resource::<AnimationPresentationFocus>()
            .init_resource::<AnimationPresentationMetrics>()
            .init_resource::<UnitAnimationStateIndex>()
            .init_resource::<UnitHpPresentationCache>()
            .add_systems(Startup, init_unit_animation_assets)
            .configure_sets(Update, UnitAnimationSystems.after(UnitRuntimeSystems))
            .add_systems(
                Update,
                (
                    build_unit_animation_graphs,
                    heal_stale_animation_player_links,
                    discover_unit_animation_players,
                    retry_pending_animation_links,
                    install_animation_graph_on_player,
                    configure_unit_animation_layering,
                    begin_death_presentations,
                    detect_unit_hit_reactions,
                    sync_unit_animation_playback,
                    tick_death_presentations,
                    tick_hit_reactions,
                    cleanup_animation_links_for_removed_roots,
                )
                    .chain()
                    .in_set(UnitAnimationSystems),
            );
    }
}

fn init_unit_animation_assets(
    catalog: Res<crate::world::UnitCatalog>,
    profiles: Res<crate::world::AnimationProfileCatalog>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.insert_resource(preload_unit_animation_gltfs(
        &catalog,
        &profiles,
        &asset_server,
    ));
}
