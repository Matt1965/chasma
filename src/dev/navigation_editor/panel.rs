//! Navigation Editor window — panel spawn and sync (Slice 7).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::widgets::{BTN_BG_IDLE, DevButtonChrome, TEXT_PRIMARY, label_text_font};
use crate::dev::window::{
    DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowRoot, DevWindowUi, TITLE_BAR_HEIGHT_PX,
    clamp_window_position, navigation_editor_body_max_height, navigation_editor_panel_width,
};

use super::layout::spawn_navigation_editor_layout;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorUi;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorContextTitle;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorContextDetails;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorStatusCard;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorStatusHeadline;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorStatusCounts;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorFloorSelector;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorFloorLabel;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorFloorUpButton;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorFloorDownButton;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRegionSelector;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRegionLabel;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRegionIndexText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRegionUpButton;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRegionDownButton;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorToolPalette;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorSelectedItemPanel;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorSelectedItemText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorColumns;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorLeftColumn;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRightColumn;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorNavRow;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorToastBanner;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorToastText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorDeleteButton;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRadiusRow;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRadiusValueText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorFloorColumn;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorRegionColumn;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorFeedbackText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorPersistenceBar;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorSummaryText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorDraftSummaryText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorGenerationSummaryText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorGenerationDetailsText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorValidationText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorOpacityRow;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorOverlayStatusText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorActionButton {
    pub action: NavigationEditorAction,
    pub disabled: bool,
}

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorOpenButton;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorSectionHeader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationEditorAction {
    InspectMode,
    EditMode,
    ExitEdit,
    FloorPrev,
    FloorNext,
    ToolSelect,
    ToolAddCorner,
    ToolAddEntrance,
    ToolAddRegion,
    ToolAddConnection,
    SelectRegionPrev,
    SelectRegionNext,
    DeleteSelection,
    RadiusUp,
    RadiusDown,
    FrameBuilding,
    ReturnCamera,
    Regenerate,
    AcceptDraft,
    ReplaceWorkingCopy,
    EditDraft,
    DiscardDraft,
    ToggleDraftPreview,
    Validate,
    SaveInstance,
    ApplyToAsset,
    ResetToAsset,
    SaveAsVariant,
    ConfirmPending,
    CancelPending,
    CancelVariant,
    CreateVariant,
    OverlayBlueprint,
    OverlayEntrances,
    OverlayBlockedArea,
    ClearRecordedPath,
}

pub fn setup_navigation_editor_panel(
    mut commands: Commands,
    bodies: Query<(Entity, &DevWindowBody)>,
) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::NavigationEditor {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevNavigationEditorUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    spawn_navigation_editor_layout(root);
                });
        });
        return;
    }
}

/// Spawn the shared "Open Navigation Editor" launcher (Selected Object + Catalog Editor tab).
pub fn spawn_open_navigation_editor_button(parent: &mut ChildSpawnerCommands, label: &str) {
    parent.spawn((
        DevNavigationEditorOpenButton,
        DevButtonChrome::default(),
        DevTooltipTarget::new(
            "Open the Navigation Editor for the selected building. Requires a placed building in world selection.",
        ),
        DevPanelUi,
        DevWindowUi,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(crate::dev::widgets::theme::BTN_BORDER_IDLE),
        BackgroundColor(BTN_BG_IDLE),
        Text::new(label),
        label_text_font(),
        TextColor(TEXT_PRIMARY),
    ));
}

/// Keep the Navigation Editor body height within the current viewport.
pub fn sync_navigation_editor_window_layout(
    mut registry: ResMut<DevWindowRegistry>,
    mut nodes: ParamSet<(
        Query<(&DevWindowRoot, &mut Node)>,
        Query<(&DevWindowBody, &mut Node)>,
    )>,
) {
    let viewport = registry.viewport;
    let panel_width = navigation_editor_panel_width(viewport);
    let body_max_height = registry
        .session(DevWindowId::NavigationEditor)
        .map(|session| navigation_editor_body_max_height(viewport, session.position.y))
        .unwrap_or_else(|| navigation_editor_body_max_height(viewport, 0.0));
    let window_height = TITLE_BAR_HEIGHT_PX + body_max_height;
    if let Some(state) = registry.session_mut(DevWindowId::NavigationEditor) {
        state.position = clamp_window_position(
            state.position,
            Vec2::new(panel_width, window_height),
            viewport,
        );
        state.computed_size = Vec2::new(panel_width, window_height);
    }
    for (root, mut node) in nodes.p0().iter_mut() {
        if root.id != DevWindowId::NavigationEditor {
            continue;
        }
        node.width = Val::Px(panel_width);
    }
    for (body, mut node) in nodes.p1().iter_mut() {
        if body.id != DevWindowId::NavigationEditor {
            continue;
        }
        node.max_height = Val::Px(body_max_height);
        node.min_height = Val::Px(0.0);
        node.overflow = Overflow::scroll_y();
    }
}

pub fn sync_open_navigation_editor_buttons(
    dev_state: Res<crate::dev::DevModeState>,
    world_selection: Res<WorldSelectionState>,
    mut buttons: Query<&mut Node, With<DevNavigationEditorOpenButton>>,
) {
    let show = dev_state.enabled
        && world_selection.category == WorldSelectionCategory::Building
        && world_selection.building_id.is_some();
    for mut node in &mut buttons {
        node.display = if show { Display::Flex } else { Display::None };
    }
}
