//! Large seller stock grid for active trade sessions.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::client::TradeSessionState;
use crate::client::{close_trade_session, seller_display_name};
use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRoot, floating_window_shell_colors,
    floating_window_shell_node, spawn_floating_title_rail, spawn_floating_window_body,
    spawn_floating_window_inner_frame,
};
use crate::ui::gameplay::inventory::{
    InventoryGridInteraction, InventoryGridPane, InventoryPaneKind, InventoryUiState,
    spawn_inventory_grid,
};
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{TEXT_MUTED, TEXT_PRIMARY, panel_body_font, panel_title_font};
use crate::world::{
    InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, UnitCatalog,
    WorldData, count_physical_gold, query_inventory_weight,
};

#[derive(Component, Debug)]
pub struct TradePanelRoot;

#[derive(Component, Debug)]
pub struct TradePanelCloseButton;

#[derive(Component, Debug)]
pub struct TradePanelHeaderText;

#[derive(Component, Debug)]
pub struct TradePanelScrollContent;

#[derive(Resource, Debug, Default)]
pub(crate) struct TradePanelContentLatch {
    revision: u64,
}

pub fn spawn_trade_panel(mut commands: Commands) {
    let (shell_bg, shell_border) = floating_window_shell_colors();
    let mut shell_node = floating_window_shell_node();
    shell_node.width = Val::Px(640.0);
    shell_node.max_width = Val::Percent(92.0);
    shell_node.max_height = Val::Percent(85.0);

    commands
        .spawn((
            TradePanelRoot,
            FloatingGameplayWindowRoot {
                id: FloatingGameplayWindowId::Trade,
            },
            PlayerHudUi,
            Button,
            Interaction::None,
            FocusPolicy::Block,
            shell_node,
            shell_bg,
            shell_border,
            Visibility::Hidden,
            ZIndex(413),
        ))
        .with_children(|root| {
            spawn_floating_window_inner_frame(root, |frame| {
                spawn_floating_title_rail(
                    frame,
                    FloatingGameplayWindowId::Trade,
                    |title| {
                        title.spawn((
                            TradePanelHeaderText,
                            Text::new("Trade"),
                            panel_title_font(),
                            TextColor(TEXT_PRIMARY),
                        ));
                    },
                    Some((TradePanelCloseButton, "Close")),
                );
                spawn_floating_window_body(frame, |body| {
                    body.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            max_height: Val::Px(520.0),
                            overflow: Overflow::scroll_y(),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        TradePanelScrollContent,
                    ));
                });
            });
        });
}

pub fn sync_trade_panel_visibility(
    trade: Res<TradeSessionState>,
    mut query: Query<(&mut Node, &mut Visibility), With<TradePanelRoot>>,
) {
    let visible = trade.open;
    for (mut node, mut visibility) in &mut query {
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn sync_trade_panel_contents(
    trade: Res<TradeSessionState>,
    world: Res<WorldData>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    unit_catalog: Res<UnitCatalog>,
    inventory_ui: Res<InventoryUiState>,
    mut latch: ResMut<TradePanelContentLatch>,
    mut commands: Commands,
    scroll: Query<Entity, With<TradePanelScrollContent>>,
    mut headers: Query<&mut Text, With<TradePanelHeaderText>>,
) {
    if !trade.open {
        latch.revision = 0;
        return;
    }
    let seller_unit = match trade.seller_unit_id {
        Some(id) => id,
        None => return,
    };
    let buyer_unit = match trade.buyer_unit_id {
        Some(id) => id,
        None => return,
    };
    let seller_name = seller_display_name(world.as_ref(), &unit_catalog, seller_unit);
    let buyer_gold = world
        .get_unit(buyer_unit)
        .and_then(|u| u.inventory_id)
        .and_then(|id| world.inventory_store().get(id))
        .map(|record| count_physical_gold(record))
        .unwrap_or(0);
    if let Ok(mut header) = headers.single_mut() {
        **header = format!("{seller_name} — Stock\nYour gold: {buyer_gold}");
    }

    let needs_rebuild = latch.revision != trade.content_revision
        || trade.is_changed()
        || world.is_changed()
        || inventory_ui.is_changed();
    if !needs_rebuild {
        return;
    }
    latch.revision = trade.content_revision;

    let Ok(scroll_entity) = scroll.single() else {
        return;
    };
    commands.entity(scroll_entity).despawn_children();

    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let instance_store = world.item_instance_store();

    commands.entity(scroll_entity).with_children(|parent| {
        parent.spawn((
            Text::new(
                "Drag items to your inventory to buy. Drag owned items here to sell.",
            ),
            panel_body_font(),
            TextColor(TEXT_MUTED),
        ));

        if let Some(seller_inventory) = trade.seller_inventory_id {
            if let Some(record) = world.inventory_store().get(seller_inventory) {
                let weight = query_inventory_weight(record, &ctx)
                    .map(|w| format!("{:.1} kg", w.total_mass_grams as f32 / 1000.0))
                    .unwrap_or_else(|_| "Weight unavailable".into());
                parent.spawn((
                    Text::new(weight),
                    panel_body_font(),
                    TextColor(TEXT_MUTED),
                ));
                parent
                    .spawn((
                        InventoryGridPane {
                            inventory_id: seller_inventory,
                            pane_kind: InventoryPaneKind::TradeSeller,
                        },
                        PlayerHudUi,
                        Button,
                        Interaction::None,
                        Node {
                            flex_shrink: 0.0,
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                    ))
                    .with_children(|grid| {
                        spawn_inventory_grid(
                            grid,
                            record,
                            seller_inventory,
                            &items,
                            instance_store,
                            InventoryGridInteraction::Interactive {
                                pane_kind: InventoryPaneKind::TradeSeller,
                            },
                            Some(&inventory_ui),
                        );
                    });
            } else {
                parent.spawn((
                    Text::new("Seller stock unavailable."),
                    panel_body_font(),
                    TextColor(TEXT_MUTED),
                ));
            }
        } else {
            parent.spawn((
                Text::new("No stock available (empty)."),
                panel_body_font(),
                TextColor(TEXT_MUTED),
            ));
        }
    });
}

pub fn handle_trade_panel_close(
    mut trade: ResMut<TradeSessionState>,
    mut inventory_ui: ResMut<InventoryUiState>,
    buttons: Query<&Interaction, With<TradePanelCloseButton>>,
) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        close_trade_session(&mut trade, &mut inventory_ui);
    }
}

pub fn reconcile_trade_panel(
    mut trade: ResMut<TradeSessionState>,
    mut inventory_ui: ResMut<InventoryUiState>,
    world: Res<WorldData>,
    authored: Res<crate::world::relationship::AuthoredRelationshipCatalog>,
) {
    crate::client::reconcile_trade_session(
        &mut trade,
        &mut inventory_ui,
        world.as_ref(),
        &authored,
    );
}
