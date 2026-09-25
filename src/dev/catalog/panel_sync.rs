//! Catalog chrome sync — tab visibility and contextual placement (Slice 4).

use bevy::ecs::system::ParamSet;

use bevy::prelude::*;

use super::components::{
    DevCatalogStatusText, DevContextualPlacementAction, DevContextualPlacementButton,
    DevContextualPlacementSection, DevPlacementActiveBanner, DevTabChrome,
};
use super::placement_controls::PlacementControlField;

use super::placement_controls::{
    PlacementUiContext, placement_control_set, placement_status_line, placement_ui_context,
};

use super::state::{on_tab_selected, tab_is_visible};

use super::tabs::tab_label;

use crate::dev::dev_mode::DevTab;
use crate::dev::input::DevPanelUi;

/// Sync tab visibility and contextual placement controls.

pub fn sync_dev_catalog_chrome(
    mut dev_state: ResMut<crate::dev::dev_mode::DevModeState>,

    registry: Res<crate::dev::window::DevWindowRegistry>,

    building_catalog: Res<crate::world::BuildingCatalog>,

    doodad_catalog: Res<crate::world::DoodadCatalog>,

    mut chrome: ParamSet<(
        Query<(&DevTabChrome, &mut Visibility, &mut Node), With<DevPanelUi>>,
        Query<&mut Node, (With<DevContextualPlacementSection>, Without<DevTabChrome>)>,
        Query<
            (&DevContextualPlacementButton, &mut Visibility, &mut Node),
            Without<DevTabChrome>,
        >,
    )>,

    mut texts: ParamSet<(
        Query<
            &mut Text,
            (
                With<DevCatalogStatusText>,
                Without<DevPlacementActiveBanner>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevPlacementActiveBanner>,
                Without<DevCatalogStatusText>,
            ),
        >,
    )>,
) {
    let catalog_open =
        dev_state.enabled && registry.is_visible(crate::dev::window::DevWindowId::Catalog);

    if !catalog_open {
        for (_tab_button, mut visibility, mut node) in chrome.p0().iter_mut() {
            *visibility = Visibility::Hidden;

            node.display = Display::None;
        }

        if let Ok(mut node) = chrome.p1().single_mut() {
            node.display = Display::None;
        }

        for (_button, mut visibility, mut node) in chrome.p2().iter_mut() {
            *visibility = Visibility::Hidden;
            node.display = Display::None;
        }

        return;
    }

    dev_state.catalog.tick_status_ttl();

    for (tab_button, mut visibility, mut node) in chrome.p0().iter_mut() {
        let visible = tab_is_visible(tab_button.tab);

        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };

        node.border = UiRect::ZERO;
    }

    let building_def = dev_state.selected_definition.as_ref().and_then(|id| {
        if let crate::dev::dev_mode::DefinitionId::Building(bid) = id {
            building_catalog.get(bid)
        } else {
            None
        }
    });

    let doodad_def = dev_state.selected_definition.as_ref().and_then(|id| {
        if let crate::dev::dev_mode::DefinitionId::Doodad(did) = id {
            doodad_catalog.get(did)
        } else {
            None
        }
    });

    let ctx = placement_ui_context(dev_state.active_tab, &dev_state);

    let controls = placement_control_set(ctx, dev_state.brush.mode, building_def, doodad_def);

    let show_placement = matches!(
        ctx,
        PlacementUiContext::Unit | PlacementUiContext::Doodad | PlacementUiContext::Building
    );

    if let Ok(mut node) = chrome.p1().single_mut() {
        node.display = if show_placement {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (button, mut visibility, mut node) in chrome.p2().iter_mut() {
        let show = field_visible(button.field, &controls);

        *visibility = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        // Visibility::Hidden still reserves flex space; collapse inactive controls.
        node.display = if show {
            Display::Flex
        } else {
            Display::None
        };
    }

    let status_line = if !dev_state.catalog.status_message.is_empty() {
        dev_state.catalog.status_message.clone()
    } else {
        placement_status_line(&dev_state, None, None)
    };

    if let Ok(mut text) = texts.p0().single_mut() {
        **text = if show_placement || !status_line.is_empty() {
            status_line
        } else if matches!(ctx, PlacementUiContext::EmptyHint) {
            "Select a definition to place".into()
        } else {
            String::new()
        };
    }

    let show_banner = dev_state.placement_tool_active()
        && !matches!(
            dev_state.active_tab,
            DevTab::Units | DevTab::Doodads | DevTab::Buildings
        );

    if let Ok(mut text) = texts.p1().single_mut() {
        if show_banner {
            **text = placement_status_line(&dev_state, None, None);
        } else {
            **text = String::new();
        }
    }
}

fn field_visible(
    field: PlacementControlField,

    controls: &super::placement_controls::PlacementControlSet,
) -> bool {
    

    match field {
        PlacementControlField::Pattern => controls.pattern,

        PlacementControlField::Count => controls.count,

        PlacementControlField::Spacing => controls.spacing,

        PlacementControlField::Radius => controls.radius,

        PlacementControlField::GridColumns => controls.grid_columns,

        PlacementControlField::GridRows => controls.grid_rows,

        PlacementControlField::Affiliation => controls.affiliation,

        PlacementControlField::Rotation => controls.rotation,

        PlacementControlField::Scale => controls.scale,
    }
}

/// Record tab selection for session memory.

/// Keep placement control labels in sync (team affiliation display).
pub fn sync_catalog_placement_button_labels(
    dev_state: Res<crate::dev::dev_mode::DevModeState>,
    registry: Res<crate::dev::window::DevWindowRegistry>,
    mut buttons: Query<(&DevContextualPlacementButton, &mut Text), With<DevPanelUi>>,
) {
    if !dev_state.enabled || !registry.is_visible(crate::dev::window::DevWindowId::Catalog) {
        return;
    }
    for (button, mut text) in &mut buttons {
        if button.action == DevContextualPlacementAction::CycleSpawnTeam
            && button.field == PlacementControlField::Affiliation
        {
            **text = dev_state.spawn_team_button_label();
        }
    }
}

pub fn track_catalog_tab_selection(
    mut dev_state: ResMut<crate::dev::dev_mode::DevModeState>,

    tabs: Query<(&DevTabChrome, &Interaction), Changed<Interaction>>,
) {
    if !dev_state.enabled {
        return;
    }

    for (button, interaction) in &tabs {
        if *interaction == Interaction::Pressed {
            on_tab_selected(&mut dev_state.catalog, button.tab);
        }
    }
}

pub fn all_catalog_tabs() -> &'static [DevTab] {
    super::state::visible_tabs()
}

pub fn spawn_tab_label(tab: DevTab) -> String {
    tab_label(tab).to_string()
}
