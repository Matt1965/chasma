//! Building scene presentation: lifecycle tint and optional scene-tag discovery (ADR-095 BA1).

use bevy::prelude::*;

use crate::world::{BuildingLifecycleState, WorldData};

use super::components::{
    BuildingDiagnosticFallback, BuildingLifecycleTintApplied, BuildingRenderEntity,
    BuildingSceneRoot, BuildingSceneTags,
};
use super::fallback::BuildingFallbackAssets;
use super::placeholder::diagnostic_fallback_color;
use super::scene_materials::prepare_scene_materials;

const SPACE_TAG_PREFIX: &str = "space:";
const ROOF_TAG_PREFIX: &str = "roof:";

/// Discover optional `space:` / `roof:` tagged scene descendants once after spawn.
pub fn discover_building_scene_tags(
    mut commands: Commands,
    roots: Query<
        (Entity, &BuildingRenderEntity),
        (With<BuildingSceneRoot>, Without<BuildingSceneTags>),
    >,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (root, _) in &roots {
        let mut tags = BuildingSceneTags::default();
        collect_scene_tags(root, &children, &names, &mut tags);
        commands.entity(root).insert(tags);
    }
}

fn collect_scene_tags(
    entity: Entity,
    children: &Query<&Children>,
    names: &Query<&Name>,
    tags: &mut BuildingSceneTags,
) {
    if let Ok(name) = names.get(entity) {
        let label = name.as_str();
        if let Some(space_id) = label.strip_prefix(SPACE_TAG_PREFIX) {
            tags.space_node_names.push(space_id.to_owned());
        } else if label.starts_with(ROOF_TAG_PREFIX) {
            tags.roof_entities.push(entity);
        }
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            collect_scene_tags(child, children, names, tags);
        }
    }
}

/// Apply lifecycle styling to cloned GLB scene materials when construction state changes.
pub fn apply_building_lifecycle_tints(
    world: Res<WorldData>,
    mut commands: Commands,
    roots: Query<
        (
            Entity,
            &BuildingRenderEntity,
            Option<&BuildingLifecycleTintApplied>,
        ),
        With<BuildingSceneRoot>,
    >,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (root, marker, applied) in &roots {
        if applied.is_some_and(|value| value.lifecycle_state == marker.lifecycle_state) {
            continue;
        }
        let Some(record) = world.get_building(marker.building_id) else {
            continue;
        };
        if !prepare_scene_materials(
            &mut commands,
            root,
            &children,
            &mesh_materials,
            &mut materials,
            marker.lifecycle_state,
            record.ownership.affiliation,
        ) {
            continue;
        }
        commands.entity(root).insert(BuildingLifecycleTintApplied {
            lifecycle_state: marker.lifecycle_state,
        });
    }
}

/// Update diagnostic fallback material colors when lifecycle changes.
pub fn sync_building_fallback_materials(
    world: Res<WorldData>,
    markers: Query<
        (&BuildingRenderEntity, &MeshMaterial3d<StandardMaterial>),
        With<BuildingDiagnosticFallback>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fallback_assets: ResMut<BuildingFallbackAssets>,
) {
    for (marker, material_handle) in &markers {
        let Some(record) = world.get_building(marker.building_id) else {
            continue;
        };
        let desired = fallback_assets.material_for_state(
            &mut materials,
            marker.lifecycle_state,
            record.ownership.affiliation,
        );
        if material_handle.0 == desired {
            continue;
        }
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color =
                diagnostic_fallback_color(marker.lifecycle_state, record.ownership.affiliation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_and_roof_prefixes_are_documented() {
        assert_eq!(SPACE_TAG_PREFIX, "space:");
        assert_eq!(ROOF_TAG_PREFIX, "roof:");
    }
}
