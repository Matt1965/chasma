//! Navigation Editor structured layout spawn helpers (IN-10 / IN-10a).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::{DevTooltipContent, DevTooltipTarget};
use crate::dev::widgets::spawn_bounded_slider_row;
use crate::dev::widgets::{
    CARD_BG, CARD_BORDER, DevButtonChrome, DevButtonKind, DevCollapsibleSectionId,
    DevStatusSeverity, DevWidgetStatusLine, SPACE_CONTROL, SPACE_SECTION, TEXT_LABEL, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECTION, label_text_font, small_text_font, spawn_collapsible_section,
    standard_button_node,
};
use crate::dev::window::DevWindowUi;

use super::opacity::NAV_EDITOR_BUILDING_OPACITY_FIELD_ID;
use super::panel::{
    DevNavigationEditorActionButton, DevNavigationEditorColumns, DevNavigationEditorContextDetails,
    DevNavigationEditorContextTitle, DevNavigationEditorDeleteButton,
    DevNavigationEditorDraftSummaryText, DevNavigationEditorFloorColumn,
    DevNavigationEditorFloorDownButton, DevNavigationEditorFloorLabel,
    DevNavigationEditorFloorSelector, DevNavigationEditorFloorUpButton,
    DevNavigationEditorGenerationDetailsText, DevNavigationEditorGenerationSummaryText,
    DevNavigationEditorLeftColumn, DevNavigationEditorNavRow, DevNavigationEditorOpacityRow,
    DevNavigationEditorOverlayStatusText, DevNavigationEditorPersistenceBar,
    DevNavigationEditorRadiusRow, DevNavigationEditorRadiusValueText,
    DevNavigationEditorRegionColumn, DevNavigationEditorRegionDownButton,
    DevNavigationEditorRegionIndexText, DevNavigationEditorRegionLabel,
    DevNavigationEditorRegionSelector, DevNavigationEditorRegionUpButton,
    DevNavigationEditorRightColumn, DevNavigationEditorSectionHeader,
    DevNavigationEditorSelectedItemPanel, DevNavigationEditorSelectedItemText,
    DevNavigationEditorStatusCard, DevNavigationEditorStatusCounts,
    DevNavigationEditorStatusHeadline, DevNavigationEditorSummaryText,
    DevNavigationEditorToastBanner, DevNavigationEditorToastText, DevNavigationEditorToolPalette,
    DevNavigationEditorValidationText, NavigationEditorAction,
};

pub fn spawn_navigation_editor_layout(root: &mut ChildSpawnerCommands<'_>) {
    spawn_toast_banner(root);

    root.spawn((
        DevNavigationEditorColumns,
        DevPanelUi,
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(SPACE_SECTION),
            row_gap: Val::Px(SPACE_SECTION),
            align_items: AlignItems::FlexStart,
            ..default()
        },
    ))
    .with_children(|columns| {
        columns
            .spawn((
                DevNavigationEditorLeftColumn,
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(SPACE_CONTROL),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: Val::Percent(50.0),
                    min_width: Val::Px(0.0),
                    ..default()
                },
            ))
            .with_children(|left| {
                spawn_section_header(left, "Building");
                spawn_context_card(left);
                spawn_section_header(left, "Draft & validation");
                spawn_status_card(left);
                spawn_section_header(left, "Tools");
                spawn_tool_palette(left);
                spawn_section_header(left, "Selection");
                spawn_selected_item_panel(left);
                spawn_section_header(left, "Persistence");
                spawn_persistence_bar(left);
                spawn_hidden_variant_confirm_actions(left);
            });

        columns
            .spawn((
                DevNavigationEditorRightColumn,
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(SPACE_CONTROL),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: Val::Percent(50.0),
                    min_width: Val::Px(0.0),
                    ..default()
                },
            ))
            .with_children(|right| {
                spawn_section_header(right, "Navigation");
                spawn_nav_row(right);
                spawn_opacity_row(right);
                spawn_collapsible_validation(right);
                spawn_collapsible_generation(right);
                spawn_section_header(right, "View");
                spawn_view_actions(right);
                spawn_section_header(right, "Overlays");
                spawn_overlay_actions(right);
                spawn_overlay_status(right);
            });
    });
}

fn spawn_toast_banner(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevNavigationEditorToastBanner,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(SPACE_CONTROL), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::bottom(Val::Px(SPACE_CONTROL)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(CARD_BG),
            BorderColor::all(CARD_BORDER),
        ))
        .with_children(|banner| {
            banner.spawn((
                DevNavigationEditorToastText,
                DevWidgetStatusLine {
                    severity: DevStatusSeverity::Info,
                },
                DevPanelUi,
                Text::new(""),
                small_text_font(),
                TextColor(DevStatusSeverity::Info.color()),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
        });
}

fn spawn_section_header(parent: &mut ChildSpawnerCommands<'_>, title: &str) {
    parent.spawn((
        DevNavigationEditorSectionHeader,
        DevPanelUi,
        Text::new(title),
        section_text_font(),
        TextColor(TEXT_SECTION),
        Node {
            margin: UiRect::top(Val::Px(SPACE_SECTION)),
            ..default()
        },
    ));
}

fn section_text_font() -> TextFont {
    TextFont {
        font_size: crate::dev::widgets::theme::FONT_SIZE_SECTION,
        ..default()
    }
}

fn spawn_context_card(parent: &mut ChildSpawnerCommands<'_>) {
    parent.spawn(card_node()).with_children(|card| {
        card.spawn((
            DevNavigationEditorContextTitle,
            DevPanelUi,
            Text::new("Select a placed building"),
            label_text_font(),
            TextColor(TEXT_PRIMARY),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
        ));
        card.spawn((
            DevNavigationEditorContextDetails,
            DevPanelUi,
            Text::new(""),
            small_text_font(),
            TextColor(TEXT_MUTED),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
        ));
        card.spawn((
            DevNavigationEditorSummaryText,
            DevPanelUi,
            Node {
                display: Display::None,
                ..default()
            },
            Text::new(""),
        ));
    });
}

fn spawn_status_card(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((DevNavigationEditorStatusCard, card_node()))
        .with_children(|card| {
            card.spawn((
                DevNavigationEditorStatusHeadline,
                DevPanelUi,
                Text::new(""),
                label_text_font(),
                TextColor(TEXT_PRIMARY),
            ));
            card.spawn((
                DevNavigationEditorStatusCounts,
                DevPanelUi,
                Text::new(""),
                small_text_font(),
                TextColor(TEXT_MUTED),
            ));
            card.spawn((
                DevNavigationEditorDraftSummaryText,
                DevPanelUi,
                Node {
                    display: Display::None,
                    ..default()
                },
                Text::new(""),
            ));
            spawn_draft_actions(card);
        });
}

fn spawn_draft_actions(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(SPACE_CONTROL),
                row_gap: Val::Px(SPACE_CONTROL),
                margin: UiRect::top(Val::Px(SPACE_CONTROL)),
                ..default()
            },
        ))
        .with_children(|row| {
            spawn_nav_button(
                row,
                "Preview draft",
                NavigationEditorAction::ToggleDraftPreview,
                DevButtonKind::Secondary,
                "Toggle overlay of generated draft regions and connections.",
            );
            spawn_nav_button(
                row,
                "Edit draft",
                NavigationEditorAction::EditDraft,
                DevButtonKind::Secondary,
                "Copy generated draft into working copy for manual editing.",
            );
            spawn_nav_button(
                row,
                "Replace working copy",
                NavigationEditorAction::ReplaceWorkingCopy,
                DevButtonKind::Destructive,
                "Replace the entire working copy with the generated draft.",
            );
            spawn_nav_button(
                row,
                "Discard draft",
                NavigationEditorAction::DiscardDraft,
                DevButtonKind::Destructive,
                "Remove generated draft without changing working copy.",
            );
            spawn_nav_button(
                row,
                "Generate draft",
                NavigationEditorAction::Regenerate,
                DevButtonKind::Secondary,
                "Build a separate reviewable draft from the building model.",
            );
        });
}

fn spawn_collapsible_generation(parent: &mut ChildSpawnerCommands<'_>) {
    spawn_collapsible_section(
        parent,
        DevCollapsibleSectionId::NavEditorGeneration,
        "Generation details",
        Some(DevTooltipContent::new(
            "Verbose entrance markers and regeneration diagnostics.",
        )),
        |body| {
            body.spawn((
                DevNavigationEditorGenerationSummaryText,
                DevPanelUi,
                Node {
                    display: Display::None,
                    ..default()
                },
                Text::new(""),
            ));
            body.spawn((
                DevNavigationEditorGenerationDetailsText,
                DevPanelUi,
                Text::new(""),
                small_text_font(),
                TextColor(TEXT_MUTED),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
        },
    );
}

fn spawn_collapsible_validation(parent: &mut ChildSpawnerCommands<'_>) {
    spawn_collapsible_section(
        parent,
        DevCollapsibleSectionId::NavEditorValidation,
        "Validation findings",
        Some(DevTooltipContent::new(
            "Blueprint validation diagnostics for the working copy or generated draft.",
        )),
        |body| {
            body.spawn((
                DevNavigationEditorValidationText,
                DevPanelUi,
                Text::new(""),
                small_text_font(),
                TextColor(TEXT_MUTED),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
        },
    );
}

fn spawn_opacity_row(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevNavigationEditorOpacityRow,
            DevPanelUi,
            DevWindowUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|row| {
            spawn_bounded_slider_row(
                row,
                "Building opacity",
                NAV_EDITOR_BUILDING_OPACITY_FIELD_ID,
                110.0,
                "Editor presentation only - fades the selected building mesh.",
            );
        });
}

fn spawn_nav_row(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevNavigationEditorNavRow,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(SPACE_CONTROL),
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|row| {
            spawn_compact_floor_selector(row);
            spawn_compact_region_selector(row);
        });
}

fn compact_selector_shell() -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            flex_basis: Val::Percent(50.0),
            min_width: Val::Px(0.0),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(CARD_BG),
        BorderColor::all(CARD_BORDER),
    )
}

fn spawn_compact_floor_selector(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevNavigationEditorFloorSelector,
            DevNavigationEditorFloorColumn,
            compact_selector_shell(),
        ))
        .with_children(|col| {
            col.spawn((
                DevPanelUi,
                Text::new("FLOOR"),
                small_text_font(),
                TextColor(TEXT_LABEL),
            ));
            col.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                },
            ))
            .with_children(|controls| {
                controls
                    .spawn(nav_button_bundle(
                        "^",
                        NavigationEditorAction::FloorPrev,
                        DevButtonKind::Normal,
                    ))
                    .insert(DevNavigationEditorFloorUpButton);
                controls.spawn((
                    DevNavigationEditorFloorLabel,
                    DevPanelUi,
                    Text::new("-"),
                    small_text_font(),
                    TextColor(TEXT_PRIMARY),
                ));
                controls
                    .spawn(nav_button_bundle(
                        "v",
                        NavigationEditorAction::FloorNext,
                        DevButtonKind::Normal,
                    ))
                    .insert(DevNavigationEditorFloorDownButton);
            });
        });
}

fn spawn_compact_region_selector(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevNavigationEditorRegionSelector,
            DevNavigationEditorRegionColumn,
            compact_selector_shell(),
        ))
        .with_children(|col| {
            col.spawn((
                DevPanelUi,
                Text::new("REGION"),
                small_text_font(),
                TextColor(TEXT_LABEL),
            ));
            col.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                },
            ))
            .with_children(|controls| {
                controls
                    .spawn(nav_button_bundle(
                        "<",
                        NavigationEditorAction::SelectRegionPrev,
                        DevButtonKind::Normal,
                    ))
                    .insert(DevNavigationEditorRegionUpButton);
                controls.spawn((
                    DevNavigationEditorRegionLabel,
                    DevPanelUi,
                    Text::new("-"),
                    small_text_font(),
                    TextColor(TEXT_PRIMARY),
                    Node {
                        min_width: Val::Px(72.0),
                        ..default()
                    },
                ));
                controls
                    .spawn(nav_button_bundle(
                        ">",
                        NavigationEditorAction::SelectRegionNext,
                        DevButtonKind::Normal,
                    ))
                    .insert(DevNavigationEditorRegionDownButton);
            });
            col.spawn((
                DevNavigationEditorRegionIndexText,
                DevPanelUi,
                Text::new(""),
                small_text_font(),
                TextColor(TEXT_MUTED),
            ));
            spawn_nav_button(
                col,
                "+ Region",
                NavigationEditorAction::ToolAddRegion,
                DevButtonKind::Secondary,
                "Add a new walkable region on the active floor.",
            );
        });
}

fn spawn_tool_palette(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevNavigationEditorToolPalette,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(SPACE_CONTROL),
                row_gap: Val::Px(SPACE_CONTROL),
                ..default()
            },
        ))
        .with_children(|palette| {
            for (label, action, tip) in [
                (
                    "Select",
                    NavigationEditorAction::ToolSelect,
                    "Select and drag corners, entrances, or transitions.",
                ),
                (
                    "+ Vertex",
                    NavigationEditorAction::ToolAddCorner,
                    "Place one corner on a walkable edge.",
                ),
                (
                    "+ Entry",
                    NavigationEditorAction::ToolAddEntrance,
                    "Place an exterior portal entrance.",
                ),
                (
                    "+ Link",
                    NavigationEditorAction::ToolAddConnection,
                    "Click source region then destination.",
                ),
            ] {
                spawn_nav_button(palette, label, action, DevButtonKind::Normal, tip);
            }
        });
}

fn spawn_selected_item_panel(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((DevNavigationEditorSelectedItemPanel, card_node()))
        .with_children(|card| {
            card.spawn((
                DevNavigationEditorSelectedItemText,
                DevPanelUi,
                Text::new("Nothing selected"),
                small_text_font(),
                TextColor(TEXT_MUTED),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
            let mut delete_button = card.spawn(nav_button_bundle(
                "Delete",
                NavigationEditorAction::DeleteSelection,
                DevButtonKind::Destructive,
            ));
            delete_button.insert(DevNavigationEditorDeleteButton);
            delete_button.insert(DevTooltipTarget::from_content(DevTooltipContent::new(
                "Delete the selected feature.",
            )));
            card.spawn((
                DevNavigationEditorRadiusRow,
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(SPACE_CONTROL),
                    margin: UiRect::top(Val::Px(SPACE_CONTROL)),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|row| {
                row.spawn((
                    DevNavigationEditorRadiusValueText,
                    DevPanelUi,
                    Text::new(""),
                    small_text_font(),
                    TextColor(TEXT_MUTED),
                ));
                spawn_nav_button(
                    row,
                    "Radius -",
                    NavigationEditorAction::RadiusDown,
                    DevButtonKind::Normal,
                    "Decrease radius.",
                );
                spawn_nav_button(
                    row,
                    "Radius +",
                    NavigationEditorAction::RadiusUp,
                    DevButtonKind::Normal,
                    "Increase radius.",
                );
            });
        });
}

fn spawn_view_actions(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(SPACE_CONTROL),
                row_gap: Val::Px(SPACE_CONTROL),
                ..default()
            },
        ))
        .with_children(|row| {
            for (label, action, tip) in [
                (
                    "Inspect",
                    NavigationEditorAction::InspectMode,
                    "Read-only blueprint view.",
                ),
                (
                    "Edit",
                    NavigationEditorAction::EditMode,
                    "Edit walkable outline and entrances.",
                ),
                (
                    "Exit edit",
                    NavigationEditorAction::ExitEdit,
                    "Return to inspect mode.",
                ),
                (
                    "Frame",
                    NavigationEditorAction::FrameBuilding,
                    "Center camera on building.",
                ),
                (
                    "Return view",
                    NavigationEditorAction::ReturnCamera,
                    "Restore pre-inspection camera.",
                ),
                (
                    "Validate",
                    NavigationEditorAction::Validate,
                    "Run blueprint validation.",
                ),
            ] {
                spawn_nav_button(row, label, action, DevButtonKind::Secondary, tip);
            }
        });
}

fn spawn_overlay_status(parent: &mut ChildSpawnerCommands<'_>) {
    parent.spawn((
        DevNavigationEditorOverlayStatusText,
        DevPanelUi,
        DevWidgetStatusLine {
            severity: DevStatusSeverity::Info,
        },
        Text::new(""),
        small_text_font(),
        TextColor(TEXT_MUTED),
    ));
}

fn spawn_overlay_actions(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(SPACE_CONTROL),
                row_gap: Val::Px(SPACE_CONTROL),
                ..default()
            },
        ))
        .with_children(|row| {
            for (label, action, tip) in [
                (
                    "Authored Blueprint",
                    NavigationEditorAction::OverlayBlueprint,
                    "Show persisted or working-copy blueprint geometry (not runtime activation).",
                ),
                (
                    "Runtime Entrances",
                    NavigationEditorAction::OverlayEntrances,
                    "Show activated runtime portal triggers for the selected building.",
                ),
                (
                    "Blocked Area",
                    NavigationEditorAction::OverlayBlockedArea,
                    "Show cells blocked by the actual navigation authority (blueprint boundaries or legacy footprints).",
                ),
                (
                    "Clear Recorded Path",
                    NavigationEditorAction::ClearRecordedPath,
                    "Clear retained unit path diagnostic traces.",
                ),
            ] {
                spawn_nav_button(row, label, action, DevButtonKind::Normal, tip);
            }
        });
}

fn spawn_persistence_bar(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevNavigationEditorPersistenceBar,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(SPACE_CONTROL),
                ..default()
            },
        ))
        .with_children(|bar| {
            bar.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(SPACE_CONTROL),
                    row_gap: Val::Px(SPACE_CONTROL),
                    ..default()
                },
            ))
            .with_children(|row| {
                spawn_nav_button(
                    row,
                    "Save instance",
                    NavigationEditorAction::SaveInstance,
                    DevButtonKind::Primary,
                    "Persist override for this placed building.",
                );
                spawn_nav_button(
                    row,
                    "Apply to asset",
                    NavigationEditorAction::ApplyToAsset,
                    DevButtonKind::Secondary,
                    "Update shared asset-default blueprint.",
                );
            });
            bar.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(SPACE_CONTROL),
                    row_gap: Val::Px(SPACE_CONTROL),
                    ..default()
                },
            ))
            .with_children(|row| {
                spawn_nav_button(
                    row,
                    "Reset to asset",
                    NavigationEditorAction::ResetToAsset,
                    DevButtonKind::Destructive,
                    "Remove instance override.",
                );
                spawn_nav_button(
                    row,
                    "Save As Variant",
                    NavigationEditorAction::SaveAsVariant,
                    DevButtonKind::Secondary,
                    "Create new building definition variant.",
                );
            });
        });
}

fn spawn_hidden_variant_confirm_actions(parent: &mut ChildSpawnerCommands<'_>) {
    for (label, action, kind) in [
        (
            "Create variant",
            NavigationEditorAction::CreateVariant,
            DevButtonKind::Primary,
        ),
        (
            "Confirm",
            NavigationEditorAction::ConfirmPending,
            DevButtonKind::Primary,
        ),
        (
            "Cancel",
            NavigationEditorAction::CancelPending,
            DevButtonKind::Secondary,
        ),
        (
            "Cancel variant",
            NavigationEditorAction::CancelVariant,
            DevButtonKind::Secondary,
        ),
    ] {
        spawn_nav_button(parent, label, action, kind, "");
    }
}

fn card_node() -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(SPACE_CONTROL),
            padding: UiRect::all(Val::Px(SPACE_CONTROL)),
            border: UiRect::all(Val::Px(1.0)),
            width: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(CARD_BG),
        BorderColor::all(CARD_BORDER),
    )
}

fn nav_button_bundle(
    label: &str,
    action: NavigationEditorAction,
    kind: DevButtonKind,
) -> impl Bundle {
    let mut node = standard_button_node(6.0, 3.0);
    node.display = Display::None;
    (
        DevNavigationEditorActionButton {
            action,
            disabled: false,
        },
        DevButtonChrome {
            kind,
            disabled: false,
            active: false,
        },
        DevPanelUi,
        DevWindowUi,
        Button,
        node,
        BorderColor::all(crate::dev::widgets::theme::BTN_BORDER_IDLE),
        BackgroundColor(crate::dev::widgets::theme::BTN_BG_IDLE),
        Text::new(label),
        label_text_font(),
        TextColor(TEXT_PRIMARY),
    )
}

fn spawn_nav_button(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    action: NavigationEditorAction,
    kind: DevButtonKind,
    tooltip: &str,
) {
    let mut entity = parent.spawn(nav_button_bundle(label, action, kind));
    if !tooltip.is_empty() {
        entity.insert(DevTooltipTarget::from_content(DevTooltipContent::new(
            tooltip,
        )));
    }
}
