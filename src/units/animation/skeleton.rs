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
    #[cfg(feature = "dev")] weapons: Res<crate::world::WeaponCatalog>,
    #[cfg(feature = "dev")] mut identity_logged: Local<HashSet<crate::world::UnitDefinitionId>>,
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
        #[cfg(feature = "dev")]
        if identity_logged.insert(definition_id.clone()) {
            log_unit_animation_runtime_identity(definition, profile, &weapons);
        }
        let Some(split_bone) = profile.layering_split_bone() else {
            info!(
                "unit animation layering: unit `{}` profile `{}` has no split bone -> FullBodyExclusive",
                definition.id.as_str(),
                profile_id.as_str()
            );
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

        let mut assignments: Vec<(AnimationTargetId, u32)> = Vec::new();
        let mut upper_targets = 0usize;
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
                upper_targets += 1;
                mask_groups::UPPER_BODY
            } else {
                mask_groups::LOWER_BODY
            };
            assignments.push((*target_id, group));
        }
        let configured_targets = assignments.len();
        let lower_targets = configured_targets - upper_targets;

        // A split bone that matches no animation target would mask every bone as lower body,
        // so upper-body (attack) clips would animate nothing. Treat that as "layering
        // unsupported" instead of silently installing an empty upper mask.
        let mode = if configured_targets > 0 && upper_targets > 0 {
            for (target_id, group) in &assignments {
                graph.add_target_to_mask_group(*target_id, *group);
            }
            UnitAnimationLayeringMode::Masked
        } else {
            assets.log_once(format!(
                "skeleton incompatible with layering for unit `{}` profile `{}` (split bone `{split_bone}` matched {upper_targets} of {configured_targets} animation targets)",
                definition.id.as_str(),
                profile_id.as_str()
            ));
            UnitAnimationLayeringMode::FullBodyExclusive
        };

        info!(
            "unit animation layering: unit `{}` profile `{}` glb `{}` split_bone `{split_bone}` targets={configured_targets} upper={upper_targets} lower={lower_targets} mode={mode:?}",
            definition.id.as_str(),
            profile_id.as_str(),
            super::assets::gltf_asset_path_for_definition(definition).unwrap_or_default(),
        );

        commands.entity(root).insert(UnitAnimationLayering { mode });
    }
}

/// Dev-only report of what this process actually consumed for one animated unit definition:
/// resolved glTF path on disk (length + FNV-1a hash), profile, split bone, idle/attack clip
/// names. Lets a manual tester confirm the running build loaded the expected asset and
/// workbook rows before judging animation behavior.
#[cfg(feature = "dev")]
fn log_unit_animation_runtime_identity(
    definition: &crate::world::UnitDefinition,
    profile: &crate::world::AnimationProfile,
    weapons: &crate::world::WeaponCatalog,
) {
    let asset_root = std::env::var("BEVY_ASSET_ROOT")
        .or_else(|_| std::env::var("CARGO_MANIFEST_DIR"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
                .unwrap_or_default()
        })
        .join("assets");
    let gltf_path = super::assets::gltf_asset_path_for_definition(definition).unwrap_or_default();
    let disk_path = asset_root.join(&gltf_path);
    let (len, hash) = match std::fs::read(&disk_path) {
        Ok(bytes) => (bytes.len(), fnv1a64(&bytes)),
        Err(_) => (0, 0),
    };
    let idle = profile
        .resolve_clip_name(crate::world::AnimationClipKey::Idle)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| "<none>".to_string());
    let split = profile.layering_split_bone().unwrap_or("<none>");
    let attack = weapons
        .get(&definition.default_weapon_id)
        .map(|w| w.animation_key.trim().to_string())
        .unwrap_or_else(|| "<no default weapon>".to_string());
    let workbook = crate::data_import::dev_design_workbook_path();
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let line = format!(
        "unit animation identity: unit `{}` glb `{}` bytes={len} fnv1a64={hash:016x} profile `{}` idle `{idle}` split_bone `{split}` attack_clip `{attack}` workbook `{}` manifest_dir `{}`",
        definition.id.as_str(),
        disk_path.display(),
        profile.id.as_str(),
        workbook.display(),
        manifest_dir.display(),
    );
    info!("{line}");
    crate::logging::append_log_line(crate::logging::DEV_STARTUP_LOG_PATH, "# unit animation", &line);
}

#[cfg(feature = "dev")]
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
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
