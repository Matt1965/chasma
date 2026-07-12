use bevy::prelude::*;

use crate::units::components::{UnitRenderEntity, UnitSceneRoot};

use super::assets::UnitAnimationAssets;
use super::components::{AnimationPlaybackPending, PendingAnimationLink, UnitAnimationPlayerLink};

/// Discover descendant [`AnimationPlayer`] entities after [`SceneRoot`] spawn (A1).
pub fn discover_unit_animation_players(
    mut commands: Commands,
    roots: Query<
        (Entity, &UnitRenderEntity),
        (With<UnitSceneRoot>, Without<UnitAnimationPlayerLink>),
    >,
    children: Query<&Children>,
    players: Query<Entity, With<AnimationPlayer>>,
    mut assets: ResMut<UnitAnimationAssets>,
) {
    for (root, marker) in &roots {
        let mut found: Vec<Entity> = Vec::new();
        collect_animation_players(root, &children, &players, &mut found);
        if found.is_empty() {
            commands.entity(root).insert(PendingAnimationLink);
            continue;
        }
        found.sort_by_key(|entity| entity.to_bits());
        if found.len() > 1 {
            assets.log_once(format!(
                "unit {} has multiple AnimationPlayer descendants; using first",
                marker.unit_id.raw()
            ));
        }
        let player_entity = found[0];
        commands.entity(root).insert((
            UnitAnimationPlayerLink { player_entity },
            AnimationPlaybackPending,
        ));
        commands.entity(root).remove::<PendingAnimationLink>();
    }
}

/// Retry discovery for roots still pending a player link (A1).
pub fn retry_pending_animation_links(
    mut commands: Commands,
    pending: Query<(Entity, &UnitRenderEntity), With<PendingAnimationLink>>,
    children: Query<&Children>,
    players: Query<Entity, With<AnimationPlayer>>,
    mut assets: ResMut<UnitAnimationAssets>,
) {
    for (root, marker) in &pending {
        let mut found: Vec<Entity> = Vec::new();
        collect_animation_players(root, &children, &players, &mut found);
        if found.is_empty() {
            continue;
        }
        found.sort_by_key(|entity| entity.to_bits());
        if found.len() > 1 {
            assets.log_once(format!(
                "unit {} has multiple AnimationPlayer descendants; using first",
                marker.unit_id.raw()
            ));
        }
        commands.entity(root).insert((
            UnitAnimationPlayerLink {
                player_entity: found[0],
            },
            AnimationPlaybackPending,
        ));
        commands.entity(root).remove::<PendingAnimationLink>();
    }
}

/// Remove stale links when render roots despawn (A1).
pub fn cleanup_animation_links_for_removed_roots(
    mut removed: RemovedComponents<UnitRenderEntity>,
    links: Query<(Entity, &UnitAnimationPlayerLink)>,
    mut commands: Commands,
) {
    for entity in removed.read() {
        if links.get(entity).is_ok() {
            commands
                .entity(entity)
                .remove::<UnitAnimationPlayerLink>()
                .remove::<PendingAnimationLink>();
        }
    }
}

/// Clear invalid player links so discovery can retry after scene subtree rebuild (A1).
pub fn heal_stale_animation_player_links(
    mut commands: Commands,
    links: Query<
        (Entity, &UnitAnimationPlayerLink),
        (
            Without<PendingAnimationLink>,
            With<super::components::UnitAnimationGraphInstalled>,
        ),
    >,
    players: Query<Entity, With<AnimationPlayer>>,
) {
    for (root, link) in &links {
        if players.get(link.player_entity).is_ok() {
            continue;
        }
        let player = link.player_entity;
        commands.entity(root).insert((
            PendingAnimationLink,
            super::components::AnimationPlaybackPending,
        ));
        commands.entity(root).remove::<(
            UnitAnimationPlayerLink,
            super::components::UnitAnimationGraphInstalled,
        )>();
        if players.get(player).is_ok() {
            commands.entity(player).remove::<(
                AnimationGraphHandle,
                AnimationTransitions,
                super::components::UnitAnimationGraphInstalled,
                super::components::AnimationProfileHandle,
            )>();
        }
    }
}

fn collect_animation_players(
    root: Entity,
    children: &Query<&Children>,
    players: &Query<Entity, With<AnimationPlayer>>,
    found: &mut Vec<Entity>,
) {
    if players.get(root).is_ok() {
        found.push(root);
    }
    if let Ok(kids) = children.get(root) {
        for child in kids.iter() {
            collect_animation_players(child, children, players, found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::animation::components::{
        AnimationPlaybackPending, UnitAnimationGraphInstalled,
    };

    #[test]
    fn stale_player_link_returns_to_pending_discovery() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, heal_stale_animation_player_links);

        let root = app
            .world_mut()
            .spawn((
                UnitAnimationPlayerLink {
                    player_entity: Entity::from_bits(999),
                },
                UnitAnimationGraphInstalled,
            ))
            .id();

        app.update();
        let entity = app.world().entity(root);
        assert!(entity.get::<PendingAnimationLink>().is_some());
        assert!(entity.get::<AnimationPlaybackPending>().is_some());
        assert!(entity.get::<UnitAnimationPlayerLink>().is_none());
        assert!(entity.get::<UnitAnimationGraphInstalled>().is_none());
    }
}
