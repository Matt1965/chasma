//! Persistent owned-building menu (BP1 shell + BP2 content).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::player::LocalPlayerOwnership;
use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRoot, FloatingWindowTitleBarDragRegion,
    TITLE_BAR_HEIGHT_PX,
};
use crate::ui::gameplay::inventory::{
    InventoryGridInteraction, InventoryGridPane, InventoryPaneSide, InventoryUiState,
    spawn_inventory_grid, spawn_read_only_inventory_grid_shell,
};
use crate::units::input::SelectedUnits;
use crate::world::BuildingOperationParams;
use crate::world::{
    BuildingCatalog, BuildingFieldRequirementCatalog, BuildingFieldRequirementCatalogRevision,
    BuildingInteractionProfileCatalog, BuildingTerrainAssessmentStore, FieldResponseProfileCatalog,
    FieldResponseProfileCatalogRevision, FootprintCatalog, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, OperationCatalog, TerrainFieldCatalog, WorldData,
};

use super::super::layout::PlayerHudUi;
use super::content::{BuildingPanelSnapshot, build_building_panel_snapshot};
use super::controls::spawn_production_controls;
use super::format::format_building_header_line;
use super::interaction::{
    building_inventory_grid_interaction, building_inventory_transfer_eligible,
    resolve_building_inventory_actor,
};
use super::state::BuildingPanelState;

#[derive(Component, Debug)]
pub struct BuildingMenuPanelRoot;

#[derive(Component, Debug)]
pub(crate) struct BuildingMenuHeaderText;

#[derive(Component, Debug)]
pub(crate) struct BuildingMenuScrollBody;

#[derive(Component, Debug)]
pub(crate) struct BuildingMenuContentHost;

#[derive(Component, Debug)]
pub struct BuildingMenuCloseButton;

#[derive(SystemParam)]
pub struct BuildingPanelSyncParams<'w> {
    pub panel: Res<'w, BuildingPanelState>,
    pub world: Res<'w, WorldData>,
    pub building_catalog: Res<'w, BuildingCatalog>,
    pub operation_catalog: Res<'w, OperationCatalog>,
    pub interaction_catalog: Res<'w, BuildingInteractionProfileCatalog>,
    pub inventory_ui: Res<'w, InventoryUiState>,
    pub selected_units: Res<'w, SelectedUnits>,
    pub items: Res<'w, ItemCatalog>,
    pub categories: Res<'w, ItemCategoryCatalog>,
    pub profiles: Res<'w, InventoryProfileCatalog>,
    pub field_catalog: Res<'w, TerrainFieldCatalog>,
    pub requirement_catalog: Res<'w, BuildingFieldRequirementCatalog>,
    pub profile_catalog: Res<'w, FieldResponseProfileCatalog>,
    pub footprint_catalog: Res<'w, FootprintCatalog>,
    pub requirement_revision: Res<'w, BuildingFieldRequirementCatalogRevision>,
    pub profile_revision: Res<'w, FieldResponseProfileCatalogRevision>,
    pub assessment_store: ResMut<'w, BuildingTerrainAssessmentStore>,
}

pub fn spawn_building_menu_panel(mut commands: Commands) {
    commands
        .spawn((
            BuildingMenuPanelRoot,
            FloatingGameplayWindowRoot {
                id: FloatingGameplayWindowId::BuildingMenu,
            },
            PlayerHudUi,
            Button,
            Interaction::None,
            FocusPolicy::Block,
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(super::super::styles::PANEL_PADDING_PX)),
                min_width: Val::Px(220.0),
                max_width: Val::Px(360.0),
                height: Val::Percent(65.0),
                max_height: Val::Percent(70.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(super::super::styles::PANEL_BG),
            ZIndex(410),
        ))
        .with_children(|panel| {
            panel
                .spawn((
                    FloatingWindowTitleBarDragRegion {
                        id: FloatingGameplayWindowId::BuildingMenu,
                    },
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(TITLE_BAR_HEIGHT_PX),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|title| {
                    title.spawn((
                        BuildingMenuHeaderText,
                        Text::new(""),
                        super::super::styles::hud_title_font(),
                        TextColor(super::super::styles::TEXT_PRIMARY),
                    ));
                });
            panel
                .spawn((
                    BuildingMenuScrollBody,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        overflow: Overflow::scroll_y(),
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        ..default()
                    },
                ))
                .with_children(|scroll| {
                    scroll.spawn((
                        BuildingMenuContentHost,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(10.0),
                            ..default()
                        },
                    ));
                });
            panel
                .spawn((
                    BuildingMenuCloseButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                        align_self: AlignSelf::FlexStart,
                        ..default()
                    },
                    BackgroundColor(super::super::styles::CMD_BTN_ENABLED_BG),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Close"),
                        super::super::styles::hud_body_font(),
                        TextColor(super::super::styles::TEXT_PRIMARY),
                    ));
                });
        });
}

pub fn sync_building_menu_panel(
    mut params: BuildingPanelSyncParams,
    mut roots: Query<(&mut Node, &Children), With<BuildingMenuPanelRoot>>,
    mut header: Query<&mut Text, With<BuildingMenuHeaderText>>,
    content_hosts: Query<Entity, With<BuildingMenuContentHost>>,
    mut commands: Commands,
    mut last_signature: Local<Option<(crate::world::BuildingId, u64, bool, u64)>>,
) {
    let Ok((mut root_node, _children)) = roots.single_mut() else {
        return;
    };

    let Some(building_id) = params.panel.open_building_id else {
        root_node.display = Display::None;
        *last_signature = None;
        return;
    };

    let inventory_ctx =
        crate::world::InventoryCatalogCtx::new(&params.items, &params.categories, &params.profiles);
    let mut operation_params = BuildingOperationParams {
        field_catalog: &params.field_catalog,
        requirement_catalog: &params.requirement_catalog,
        profile_catalog: &params.profile_catalog,
        footprint_catalog: &params.footprint_catalog,
        operation_catalog: &params.operation_catalog,
        inventory_ctx: &inventory_ctx,
        requirement_revision: params.requirement_revision.0,
        profile_revision: params.profile_revision.0,
        assessment_store: &mut params.assessment_store,
    };

    let Some(snapshot) = build_building_panel_snapshot(
        &params.world,
        &params.building_catalog,
        &params.operation_catalog,
        &mut operation_params,
        &params.profiles,
        building_id,
    ) else {
        root_node.display = Display::None;
        *last_signature = None;
        return;
    };

    let signature = snapshot.content_signature();
    let actor = resolve_building_inventory_actor(&params.inventory_ui, &params.selected_units);
    let interaction_eligible = actor.is_some_and(|unit_id| {
        building_inventory_transfer_eligible(
            &params.world,
            &params.building_catalog,
            &params.interaction_catalog,
            building_id,
            unit_id,
        )
    });
    let actor_key = actor.map(|id| id.raw()).unwrap_or(0);
    let sync_key = (building_id, signature, interaction_eligible, actor_key);
    if *last_signature == Some(sync_key) && !params.panel.is_changed() {
        root_node.display = Display::Flex;
        return;
    }
    *last_signature = Some(sync_key);

    if let Ok(mut label) = header.single_mut() {
        **label = format_building_header_line(
            &snapshot.header.display_name,
            &snapshot.header.lifecycle_label,
            snapshot.header.current_hp,
            snapshot.header.max_hp,
        );
    }

    let Ok(host) = content_hosts.single() else {
        root_node.display = Display::Flex;
        return;
    };

    commands.entity(host).despawn_children();
    commands.entity(host).with_children(|parent| {
        spawn_panel_content(
            parent,
            &snapshot,
            &params.world,
            &params.items,
            building_inventory_grid_interaction(interaction_eligible),
            if interaction_eligible {
                Some(params.inventory_ui.as_ref())
            } else {
                None
            },
        );
    });

    root_node.display = Display::Flex;
}

fn spawn_panel_content(
    parent: &mut ChildSpawnerCommands<'_>,
    snapshot: &BuildingPanelSnapshot,
    world: &WorldData,
    items: &ItemCatalog,
    grid_interaction: InventoryGridInteraction,
    inventory_ui: Option<&InventoryUiState>,
) {
    if let Some(production) = &snapshot.production {
        spawn_production_controls(parent, production);
    }

    let instance_store = world.item_instance_store();
    for section in &snapshot.inventories {
        parent
            .spawn((Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },))
            .with_children(|binding_section| {
                binding_section.spawn((
                    Text::new(&section.label),
                    super::super::styles::hud_body_font(),
                    TextColor(super::super::styles::TEXT_PRIMARY),
                ));
                if let Some(record) = world.inventory_store().get(section.inventory_id) {
                    let side = match grid_interaction {
                        InventoryGridInteraction::Interactive { side } => side,
                        InventoryGridInteraction::ReadOnly => InventoryPaneSide::Right,
                    };
                    binding_section
                        .spawn((
                            InventoryGridPane {
                                inventory_id: section.inventory_id,
                                side,
                            },
                            PlayerHudUi,
                            Node {
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ))
                        .with_children(|grid_host| {
                            spawn_inventory_grid(
                                grid_host,
                                record,
                                section.inventory_id,
                                items,
                                &instance_store,
                                grid_interaction,
                                inventory_ui,
                            );
                        });
                } else {
                    spawn_read_only_inventory_grid_shell(
                        binding_section,
                        section.grid_width,
                        section.grid_height,
                    );
                }
            });
    }
}

pub fn handle_building_menu_close_button(
    mut panel: ResMut<BuildingPanelState>,
    mut interactions: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<BuildingMenuCloseButton>),
    >,
) {
    for (interaction, mut bg) in &mut interactions {
        *bg = match *interaction {
            Interaction::Pressed => {
                panel.close();
                BackgroundColor(super::super::styles::CMD_BTN_ENABLED_PRESSED)
            }
            Interaction::Hovered => BackgroundColor(super::super::styles::CMD_BTN_ENABLED_HOVER),
            Interaction::None => BackgroundColor(super::super::styles::CMD_BTN_ENABLED_BG),
        };
    }
}

pub fn reconcile_building_menu_panel(
    mut panel: ResMut<BuildingPanelState>,
    world: Res<WorldData>,
    player: Res<LocalPlayerOwnership>,
) {
    super::logic::reconcile_building_panel(&mut panel, &world, &player);
}
