//! Skeleton validation and mask-group configuration (A4).

use std::collections::HashSet;

use bevy::animation::{AnimatedBy, AnimationTargetId};
use bevy::prelude::*;

use crate::units::components::{UnitRenderEntity, UnitRenderMetadata};
use crate::world::{AnimationProfileCatalog, UnitCatalog, WorldData};

use super::assets::{UnitAnimationAssets, resolve_presentation_definition_id};
use super::components::{
    DeathPresentation, UnitAnimationGraphInstalled, UnitAnimationLayering, UnitAnimationPlayerLink,
};
use super::layers::{UnitAnimationLayeringMode, bone_path_is_upper_body, mask_groups};

/// Configure mask groups on the unit animation graph once the scene skeleton is available (A4).
pub fn configure_unit_animation_layering(
    mut commands: Commands,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    profiles: Res<AnimationProfileCatalog>,
    mut assets: ResMut<UnitAnimationAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    roots: Query<
        (
            Entity,
            &UnitAnimationPlayerLink,
            Option<&UnitRenderEntity>,
            Option<&UnitRenderMetadata>,
            Option<&DeathPresentation>,
        ),
        (
            With<UnitAnimationGraphInstalled>,
            Without<UnitAnimationLayering>,
        ),
    >,
    players: Query<&AnimationGraphHandle>,
    targets: Query<(Entity, &AnimationTargetId, &AnimatedBy)>,
    names: Query<&Name>,
    child_of: Query<&ChildOf>,
    children: Query<&Children>,
) {
    for (root, link, marker, metadata, death) in &roots {
        let Some(definition_id) = resolve_presentation_definition_id(marker, metadata, &world)
        else {
            continue;
        };
        let Some(definition) = catalog.get(&definition_id) else {
            commands
                .entity(root)
                .insert(UnitAnimationLayering::full_body_exclusive());
            continue;
        };
        let profile_id = death
            .map(|value| value.profile_id.clone())
            .or_else(|| definition.animation_profile_id.clone());
        let Some(profile_id) = profile_id else {
            commands
                .entity(root)
                .insert(UnitAnimationLayering::full_body_exclusive());
            continue;
        };
        let Some(profile) = profiles.get(&profile_id) else {
            commands
                .entity(root)
                .insert(UnitAnimationLayering::full_body_exclusive());
            continue;
        };
        let Some(split_bone) = profile.layering_split_bone() else {
            commands
                .entity(root)
                .insert(UnitAnimationLayering::full_body_exclusive());
            continue;
        };

        let Ok(graph_handle) = players.get(link.player_entity) else {
            continue;
        };
        let Some(graph) = graphs.get_mut(&graph_handle.0) else {
            continue;
        };

        let descendant_entities = collect_player_descendants(root, link.player_entity, &children);

        let mut configured_targets = 0usize;
        for (target_entity, target_id, animated_by) in &targets {
            if animated_by.0 != link.player_entity {
                continue;
            }
            if !descendant_entities.contains(&target_entity) {
                continue;
            }
            let Some(path) = entity_bone_path(target_entity, &names, &child_of) else {
                continue;
            };
            let group = if bone_path_is_upper_body(&path, split_bone) {
                mask_groups::UPPER_BODY
            } else {
                mask_groups::LOWER_BODY
            };
            graph.add_target_to_mask_group(*target_id, group);
            configured_targets += 1;
        }

        let mode = if configured_targets > 0 {
            UnitAnimationLayeringMode::Masked
        } else {
            assets.log_once(format!(
                "skeleton incompatible with layering for unit `{}` profile `{}` (split bone `{split_bone}`)",
                definition.id.as_str(),
                profile_id.as_str()
            ));
            UnitAnimationLayeringMode::FullBodyExclusive
        };

        commands.entity(root).insert(UnitAnimationLayering { mode });
    }
}

fn collect_player_descendants(
    root: Entity,
    player_entity: Entity,
    children: &Query<&Children>,
) -> HashSet<Entity> {
    let mut entities = HashSet::new();
    entities.insert(root);
    entities.insert(player_entity);
    collect_descendants(player_entity, children, &mut entities);
    entities
}

fn collect_descendants(entity: Entity, children: &Query<&Children>, out: &mut HashSet<Entity>) {
    let Ok(kids) = children.get(entity) else {
        return;
    };
    for child in kids.iter() {
        out.insert(child);
        collect_descendants(child, children, out);
    }
}

fn entity_bone_path(
    mut entity: Entity,
    names: &Query<&Name>,
    child_of: &Query<&ChildOf>,
) -> Option<String> {
    let mut segments = Vec::new();
    loop {
        let Ok(name) = names.get(entity) else {
            break;
        };
        segments.push(name.as_str().to_string());
        let Ok(parent) = child_of.get(entity) else {
            break;
        };
        entity = parent.parent();
    }
    segments.reverse();
    if segments.is_empty() {
        None
    } else {
        Some(segments.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_mismatch_falls_back_to_full_body_mode() {
        let layering = UnitAnimationLayering::full_body_exclusive();
        assert_eq!(layering.mode, UnitAnimationLayeringMode::FullBodyExclusive);
    }
}
