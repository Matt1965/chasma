//! Socket bone resolution on spawned unit scene hierarchies.

use bevy::prelude::*;

use super::profile::bone_suffix_for_socket;
use crate::world::equipment::EquipmentAttachmentSocket;

/// Find a descendant bone entity whose hierarchical name path ends with `bone_suffix`.
pub fn find_socket_bone_entity(
    unit_root: Entity,
    bone_suffix: &str,
    names: &Query<&Name>,
    child_of: &Query<&ChildOf>,
    children: &Query<&Children>,
) -> Option<Entity> {
    let mut stack = vec![unit_root];
    while let Some(entity) = stack.pop() {
        if bone_path_matches_suffix(entity, bone_suffix, names, child_of) {
            return Some(entity);
        }
        if let Ok(kids) = children.get(entity) {
            for child in kids.iter() {
                stack.push(child);
            }
        }
    }
    None
}

/// Resolve a semantic socket to a bone entity on a unit render hierarchy.
pub fn resolve_socket_bone_entity(
    unit_root: Entity,
    unit_render_key: &str,
    socket: EquipmentAttachmentSocket,
    names: &Query<&Name>,
    child_of: &Query<&ChildOf>,
    children: &Query<&Children>,
) -> Option<Entity> {
    let suffix = bone_suffix_for_socket(unit_render_key, socket)?;
    find_socket_bone_entity(unit_root, suffix, names, child_of, children)
}

fn bone_path_matches_suffix(
    entity: Entity,
    suffix: &str,
    names: &Query<&Name>,
    child_of: &Query<&ChildOf>,
) -> bool {
    entity_bone_path(entity, names, child_of)
        .is_some_and(|path| path.ends_with(suffix) || path == suffix)
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
