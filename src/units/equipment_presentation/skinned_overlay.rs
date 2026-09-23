//! Skinned armor overlay rebinding onto the live unit skeleton.

use std::collections::HashMap;

use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;

use super::components::{UnitEquipmentSceneRoot, UnitEquipmentSkinnedPending, UnitEquipmentVisual};

/// Rebind imported armor skinned meshes to the live unit skeleton.
pub fn finalize_skinned_equipment_overlays(
    mut commands: Commands,
    unit_roots: Query<
        Entity,
        Or<(
            With<crate::units::components::UnitSceneRoot>,
            With<crate::corpses::CorpseSceneRoot>,
        )>,
    >,
    pending: Query<
        (Entity, &UnitEquipmentVisual, &ChildOf),
        (
            With<UnitEquipmentSkinnedPending>,
            With<UnitEquipmentSceneRoot>,
        ),
    >,
    skinned: Query<(Entity, &SkinnedMesh, &Name)>,
    names: Query<&Name>,
    child_of: Query<&ChildOf>,
    children: Query<&Children>,
    animation_players: Query<(), With<AnimationPlayer>>,
) {
    for (visual_entity, visual, parent) in &pending {
        let unit_root = parent.parent();
        if !unit_roots.contains(unit_root) {
            continue;
        }
        let unit_bones = collect_bone_entities_by_name(unit_root, &names, &child_of, &children);
        if unit_bones.is_empty() {
            continue;
        }
        let armor_entities = collect_descendants(visual_entity, &children);
        let mut rebound_any = false;
        let mut armature_roots = Vec::new();
        for entity in armor_entities {
            if animation_players.contains(entity) {
                commands.entity(entity).despawn();
                continue;
            }
            if names
                .get(entity)
                .is_ok_and(|name| name.as_str() == "Armature")
            {
                armature_roots.push(entity);
            }
            let Ok((_, skinned_mesh, mesh_name)) = skinned.get(entity) else {
                continue;
            };
            let Some(rebound) = rebind_skinned_mesh(skinned_mesh, &unit_bones, &names) else {
                warn!(
                    "equipment skinned mesh `{}` could not rebind for unit `{}` slot {:?}",
                    mesh_name,
                    visual.unit_id.raw(),
                    visual.slot
                );
                continue;
            };
            commands
                .entity(entity)
                .insert((ChildOf(visual_entity), rebound));
            rebound_any = true;
        }
        if rebound_any {
            for armature in armature_roots {
                commands.entity(armature).despawn();
            }
            commands
                .entity(visual_entity)
                .remove::<UnitEquipmentSkinnedPending>();
        }
    }
}

fn collect_descendants(root: Entity, children: &Query<&Children>) -> Vec<Entity> {
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(entity) = stack.pop() {
        out.push(entity);
        if let Ok(kids) = children.get(entity) {
            for child in kids.iter() {
                stack.push(child);
            }
        }
    }
    out
}

fn collect_bone_entities_by_name(
    unit_root: Entity,
    names: &Query<&Name>,
    child_of: &Query<&ChildOf>,
    children: &Query<&Children>,
) -> HashMap<String, Entity> {
    let mut map = HashMap::new();
    for entity in collect_descendants(unit_root, children) {
        if let Ok(name) = names.get(entity) {
            map.insert(name.as_str().to_string(), entity);
        }
    }
    let _ = child_of;
    map
}

fn rebind_skinned_mesh(
    skinned_mesh: &SkinnedMesh,
    unit_bones: &HashMap<String, Entity>,
    names: &Query<&Name>,
) -> Option<SkinnedMesh> {
    let mut joints = Vec::with_capacity(skinned_mesh.joints.len());
    for joint_entity in &skinned_mesh.joints {
        let joint_name = names.get(*joint_entity).ok()?.as_str();
        let Some(unit_joint) = unit_bones.get(joint_name) else {
            warn!("missing unit bone `{joint_name}` for armor skin rebinding");
            return None;
        };
        joints.push(*unit_joint);
    }
    Some(SkinnedMesh {
        inverse_bindposes: skinned_mesh.inverse_bindposes.clone(),
        joints,
    })
}
