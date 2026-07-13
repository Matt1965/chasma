//! Bottom-left building info when a building is selected (ADR-082 B5).

use bevy::prelude::*;

use crate::world::{BuildingCatalog, WorldData, is_building_operational};

use super::building_selection::GameplayBuildingSelection;

#[derive(Component, Debug)]
pub struct SelectedBuildingPanelRoot;

#[derive(Component, Debug)]
pub(crate) struct SelectedBuildingPanelText;

pub fn spawn_selected_building_panel(parent: &mut ChildSpawnerCommands<'_>) {
    parent.spawn((
        SelectedBuildingPanelRoot,
        SelectedBuildingPanelText,
        Text::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgba(0.85, 0.9, 0.95, 1.0)),
        Node {
            display: Display::None,
            max_width: Val::Px(220.0),
            ..default()
        },
    ));
}

pub fn sync_selected_building_panel(
    selection: Res<GameplayBuildingSelection>,
    world: Res<WorldData>,
    catalog: Res<BuildingCatalog>,
    mut text: Query<(&mut Text, &mut Node), With<SelectedBuildingPanelText>>,
) {
    let Ok((mut label, mut node)) = text.single_mut() else {
        return;
    };

    let Some(building_id) = selection.building_id else {
        node.display = Display::None;
        return;
    };

    let Some(record) = world.get_building(building_id) else {
        node.display = Display::None;
        return;
    };
    let display_name = catalog
        .get(&record.definition_id)
        .map(|def| def.display_name.as_str())
        .unwrap_or(record.definition_id.as_str());

    node.display = Display::Flex;
    **label = format!(
        "Building: {}\nState: {} ({:.0}%)\nHP: {}/{}\n{}",
        display_name,
        record.lifecycle_state.label(),
        record.construction.progress_0_1 * 100.0,
        record.vitals.current_hp,
        record.vitals.max_hp,
        if is_building_operational(record) {
            "Operational"
        } else {
            "Not operational"
        },
    );
}
