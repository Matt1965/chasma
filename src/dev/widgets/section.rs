//! Collapsible section headers and session state (Slice 9).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipContent;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::window::DevWindowUi;

use super::theme::{TEXT_SECTION, small_text_font};

/// Visual collapse indicator (font-independent bars).
#[derive(Component, Debug, Clone, Copy)]
pub struct DevCollapsibleIndicator {
    pub id: DevCollapsibleSectionId,
}

/// Horizontal bar inside the collapse indicator.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DevCollapsibleIndicatorBar;

/// Stable section identity (session-local, not scene-persisted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DevCollapsibleSectionId {
    DebugMaster,
    DebugSelection,
    DebugNavigation,
    DebugSession,
    DebugAnimation,
    WorldDayLighting,
    WorldNightLighting,
    WorldTwilight,
    WorldManualLighting,
    WorldWater,
    WorldProjectDefaults,
    WorldHarness,
    FieldsBuild,
    FieldsProbe,
    FieldsOverlays,
    NavEditorValidation,
    NavEditorGeneration,
    SelectedObjectDiagnostics,
    SelectedBuildingConstruction,
    SelectedBuildingLifecycle,
    SelectedBuildingProduction,
    SelectedBuildingInventory,
    SelectedBuildingLogistics,
    SelectedBuildingDiagnostics,
}

/// Session-expanded state for collapsible sections.
#[derive(Resource, Debug, Default, Clone)]
pub struct DevCollapsibleState {
    pub expanded: HashMap<DevCollapsibleSectionId, bool>,
}

impl DevCollapsibleSectionId {
    /// Session default when the user has not toggled this section yet.
    pub fn default_expanded(self) -> bool {
        match self {
            Self::WorldDayLighting
            | Self::WorldNightLighting
            | Self::WorldTwilight
            | Self::WorldManualLighting
            | Self::WorldProjectDefaults
            | Self::WorldHarness
            | Self::NavEditorGeneration => false,
            _ => true,
        }
    }
}

impl DevCollapsibleState {
    pub fn is_expanded(&self, id: DevCollapsibleSectionId) -> bool {
        self.expanded
            .get(&id)
            .copied()
            .unwrap_or_else(|| id.default_expanded())
    }

    pub fn set_expanded(&mut self, id: DevCollapsibleSectionId, expanded: bool) {
        self.expanded.insert(id, expanded);
    }

    pub fn toggle(&mut self, id: DevCollapsibleSectionId) {
        let next = !self.is_expanded(id);
        self.set_expanded(id, next);
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevCollapsibleSection {
    pub id: DevCollapsibleSectionId,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevCollapsibleToggleButton {
    pub id: DevCollapsibleSectionId,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevCollapsibleBody {
    pub id: DevCollapsibleSectionId,
}

#[derive(Component, Debug)]
pub struct DevCollapsibleHeaderBadge;

/// Spawn collapsible section with header and body filled by callback.
pub fn spawn_collapsible_section(
    parent: &mut ChildSpawnerCommands<'_>,
    id: DevCollapsibleSectionId,
    title: &str,
    tooltip: Option<DevTooltipContent>,
    fill_body: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    parent
        .spawn((
            DevCollapsibleSection { id },
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|section| {
            section
                .spawn((
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|header| {
                    let toggle_bundle = (
                        DevCollapsibleToggleButton { id },
                        DevCollapsibleIndicator { id },
                        DevPanelUi,
                        DevWindowUi,
                        Button,
                        Node {
                            width: Val::Px(18.0),
                            height: Val::Px(18.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(super::theme::BTN_BG_IDLE),
                        BorderColor::all(super::theme::FIELD_BORDER_IDLE),
                    );
                    if let Some(ref tip) = tooltip {
                        header
                            .spawn((DevTooltipTarget::from_content(tip.clone()), toggle_bundle))
                            .with_children(|btn| spawn_collapse_indicator_bars(btn));
                    } else {
                        header
                            .spawn(toggle_bundle)
                            .with_children(|btn| spawn_collapse_indicator_bars(btn));
                    }
                    let title_bundle = (
                        DevPanelUi,
                        Text::new(title),
                        small_text_font(),
                        TextColor(TEXT_SECTION),
                    );
                    if let Some(tip) = tooltip {
                        header.spawn((DevTooltipTarget::from_content(tip), title_bundle));
                    } else {
                        header.spawn(title_bundle);
                    }
                    header.spawn((
                        DevCollapsibleHeaderBadge,
                        DevPanelUi,
                        Text::new(""),
                        small_text_font(),
                        TextColor(super::theme::STATUS_WARNING),
                    ));
                });
            section
                .spawn((
                    DevCollapsibleBody { id },
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                ))
                .with_children(fill_body);
        });
}

pub fn sync_collapsible_sections(
    state: Res<DevCollapsibleState>,
    mut bodies: Query<(&DevCollapsibleBody, &mut Node)>,
    indicators: Query<(&DevCollapsibleIndicator, &Children)>,
    mut bars: Query<&mut Visibility, With<DevCollapsibleIndicatorBar>>,
) {
    for (body, mut node) in &mut bodies {
        node.display = if state.is_expanded(body.id) {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (indicator, children) in &indicators {
        let expanded = state.is_expanded(indicator.id);
        let child_entities: Vec<Entity> = children.iter().collect();
        if child_entities.len() < 2 {
            continue;
        }
        if let Ok(mut horizontal) = bars.get_mut(child_entities[0]) {
            *horizontal = Visibility::Visible;
        }
        if let Ok(mut vertical) = bars.get_mut(child_entities[1]) {
            *vertical = if expanded {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
}

fn spawn_collapse_indicator_bars(parent: &mut ChildSpawnerCommands<'_>) {
    parent.spawn((
        DevCollapsibleIndicatorBar,
        DevPanelUi,
        Node {
            width: Val::Px(8.0),
            height: Val::Px(2.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(TEXT_SECTION),
    ));
    parent.spawn((
        DevCollapsibleIndicatorBar,
        DevPanelUi,
        Node {
            width: Val::Px(2.0),
            height: Val::Px(8.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(TEXT_SECTION),
        Visibility::Hidden,
    ));
}

pub fn handle_collapsible_toggles(
    mut state: ResMut<DevCollapsibleState>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    buttons: Query<(&Interaction, &DevCollapsibleToggleButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        state.toggle(button.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_state_is_session_local() {
        let mut state = DevCollapsibleState::default();
        assert!(state.is_expanded(DevCollapsibleSectionId::DebugMaster));
        state.toggle(DevCollapsibleSectionId::DebugMaster);
        assert!(!state.is_expanded(DevCollapsibleSectionId::DebugMaster));
    }

    #[test]
    fn world_sections_default_collapsed() {
        let state = DevCollapsibleState::default();
        assert!(!state.is_expanded(DevCollapsibleSectionId::WorldDayLighting));
        assert!(!state.is_expanded(DevCollapsibleSectionId::WorldProjectDefaults));
    }
}
