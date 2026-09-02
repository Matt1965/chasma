//! Capability-driven visibility for Selected Object building sections.

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::dev_mode::DevModeState;
use crate::dev::inspector::{
    BuildingDevCapabilities, DevProductionOperationButton, WorldInspectorState,
};
use crate::dev::widgets::spawn_action_button;
use crate::world::{BuildingCatalog, BuildingFieldRequirementCatalog, WorldData};

use super::building_actions_ui::{
    BuildingDevSectionKind, DevBuildingActionSection, DevProductionOperationSelector,
};

#[derive(Resource, Debug, Default)]
pub struct BuildingActionUiCache {
    pub building_id: Option<crate::world::BuildingId>,
    pub operation_signature: u64,
}

pub fn sync_building_dev_action_sections(
    mut commands: Commands,
    dev_state: Res<DevModeState>,
    world_selection: Res<WorldSelectionState>,
    world: Res<WorldData>,
    building_catalog: Res<BuildingCatalog>,
    requirement_catalog: Res<BuildingFieldRequirementCatalog>,
    inspector: Res<WorldInspectorState>,
    mut cache: ResMut<BuildingActionUiCache>,
    mut queries: ParamSet<(
        Query<(&DevBuildingActionSection, &mut Node)>,
        Query<(Entity, &mut Node), With<DevProductionOperationSelector>>,
    )>,
) {
    let show_building =
        dev_state.enabled && world_selection.category == WorldSelectionCategory::Building;
    let building_id = show_building.then(|| world_selection.building_id).flatten();

    let caps = building_id.and_then(|id| {
        BuildingDevCapabilities::for_building(&world, &building_catalog, &requirement_catalog, id)
    });

    for (section, mut node) in queries.p0().iter_mut() {
        let visible = caps.is_some_and(|caps| match section.kind {
            BuildingDevSectionKind::Construction => caps.construction,
            BuildingDevSectionKind::Lifecycle => caps.lifecycle,
            BuildingDevSectionKind::Production => caps.production,
            BuildingDevSectionKind::Doors => caps.doors,
            BuildingDevSectionKind::Terrain => caps.terrain,
        });
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    let snapshot = inspector.building_snapshot.as_ref();
    if let (Some(building_id), Some(caps), Some(snapshot)) = (building_id, caps, snapshot) {
        let operations = snapshot
            .supported_operations
            .as_deref()
            .map(|list| {
                list.split(", ")
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let signature = operations.len() as u64;
        let rebuild =
            cache.building_id != Some(building_id) || cache.operation_signature != signature;

        for (entity, mut node) in queries.p1().iter_mut() {
            if !caps.production_operation_selector || operations.len() <= 1 {
                node.display = Display::None;
                if rebuild {
                    commands.entity(entity).despawn_related::<Children>();
                }
                continue;
            }
            node.display = Display::Flex;
            if rebuild {
                commands.entity(entity).despawn_related::<Children>();
                for (index, operation) in operations.iter().enumerate() {
                    commands.entity(entity).with_children(|row| {
                        spawn_action_button(
                            row,
                            operation,
                            Some("Select this production operation"),
                            DevProductionOperationButton {
                                operation_index: index,
                            },
                        );
                    });
                }
            }
        }

        cache.building_id = Some(building_id);
        cache.operation_signature = signature;
    } else {
        cache.building_id = None;
        cache.operation_signature = 0;
        for (entity, mut node) in queries.p1().iter_mut() {
            node.display = Display::None;
            commands.entity(entity).despawn_related::<Children>();
        }
    }
}
