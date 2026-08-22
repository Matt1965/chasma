//! Dev held-item cursor: catalog selection → ghost → click-to-place (inventory grid or terrain).

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::dev::dev_mode::{DevModeState, DevTextFieldFocus};
use crate::dev::inventory_tools::ops::{
    dev_effective_placement_quantity, dev_place_item_at_anchor, dev_spawn_ground_pile,
};
use crate::dev::{DevPanelHoverState, input::DevSpawnClickParams};
use crate::item_piles::ItemPilePresentationSettings;
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::{
    InventoryEntryWidget, InventoryGridCell, InventoryGridPane, InventoryPanelRoot,
    InventoryUiState,
};
use crate::units::input::{cursor_world_ray, terrain_click_to_world_position};
use crate::world::{
    InventoryCatalogCtx, InventoryId, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
    WorldData,
};

#[derive(Component, Debug)]
pub struct DevHeldItemGhostRoot;

#[derive(Component, Debug)]
pub struct DevHeldItemWorldGhost;

pub fn effective_held_quantity(dev_state: &DevModeState) -> u32 {
    if dev_state.text_focus == DevTextFieldFocus::ItemQuantity {
        dev_state
            .inventory
            .quantity_input
            .parse::<u32>()
            .unwrap_or(dev_state.inventory.quantity)
            .clamp(1, 10_000)
    } else {
        dev_state.inventory.quantity
    }
}

pub fn effective_held_placement_quantity(
    dev_state: &DevModeState,
    item: &crate::world::ItemDefinition,
) -> u32 {
    dev_effective_placement_quantity(item, effective_held_quantity(dev_state))
}

fn inventory_ui_pointer_active(interaction: Interaction) -> bool {
    matches!(interaction, Interaction::Pressed | Interaction::Hovered)
}

/// True when the open inventory panel or one of its grid regions owns the pointer.
pub fn gameplay_inventory_ui_consumes_click(
    inventory_ui: &InventoryUiState,
    grid_panes: &Query<&Interaction, With<InventoryGridPane>>,
    panel_roots: &Query<&Interaction, With<InventoryPanelRoot>>,
) -> bool {
    if !inventory_ui.open {
        return false;
    }
    let grid_pane_active = grid_panes
        .iter()
        .any(|interaction| inventory_ui_pointer_active(*interaction));
    let panel_active = panel_roots
        .iter()
        .any(|interaction| inventory_ui_pointer_active(*interaction));
    gameplay_inventory_ui_consumes_click_flags(inventory_ui.open, grid_pane_active, panel_active)
}

/// Pure seam for click-ownership tests (grid/panel pointer active while inventory open).
pub fn gameplay_inventory_ui_consumes_click_flags(
    inventory_open: bool,
    grid_pane_pointer_active: bool,
    panel_pointer_active: bool,
) -> bool {
    if !inventory_open {
        return false;
    }
    grid_pane_pointer_active || panel_pointer_active
}

/// Resolve explicit inventory grid anchor from UI interaction (not first-fit).
pub fn resolve_inventory_placement_anchor(
    inventory_ui: &InventoryUiState,
    world: &WorldData,
    grid_cells: &Query<(&Interaction, &InventoryGridCell)>,
    entry_widgets: &Query<(&Interaction, &InventoryEntryWidget)>,
) -> Option<(InventoryId, u8, u8)> {
    if !inventory_ui.open {
        return None;
    }

    for (interaction, cell) in grid_cells.iter() {
        if *interaction == Interaction::Pressed {
            return Some((cell.inventory_id, cell.x, cell.y));
        }
    }
    for (interaction, widget) in entry_widgets.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let record = world.inventory_store().get(widget.inventory_id)?;
        let entry = record.placed_entries().get(widget.entry_index)?;
        return Some((widget.inventory_id, entry.anchor_x, entry.anchor_y));
    }
    for (interaction, cell) in grid_cells.iter() {
        if *interaction == Interaction::Hovered {
            return Some((cell.inventory_id, cell.x, cell.y));
        }
    }
    None
}

pub fn handle_dev_held_item_input(
    mut params: DevSpawnClickParams,
    panel_hovered: Res<DevPanelHoverState>,
    inventory_ui: Res<InventoryUiState>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    grid_cells: Query<(&Interaction, &InventoryGridCell)>,
    entry_widgets: Query<(&Interaction, &InventoryEntryWidget)>,
    grid_panes: Query<&Interaction, With<InventoryGridPane>>,
    panel_roots: Query<&Interaction, With<InventoryPanelRoot>>,
) {
    if !params.dev_state.enabled {
        return;
    }
    let Some(item_id) = params.dev_state.dev_held_item_id() else {
        return;
    };

    if params.mouse_buttons.just_pressed(MouseButton::Right) {
        params.gate.block_gameplay_mouse = true;
        params.dev_state.clear_dev_held_item();
        params.dev_state.inventory.message = "Held item cleared".into();
        return;
    }

    if !params.mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if panel_hovered.hovered || params.gate.spawn_handled_this_frame {
        return;
    }

    params.dev_state.apply_item_quantity_input();
    let quantity = items
        .get(&item_id)
        .map(|item| effective_held_placement_quantity(&params.dev_state, item))
        .unwrap_or_else(|| effective_held_quantity(&params.dev_state));
    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);

    if let Some((inventory_id, anchor_x, anchor_y)) = resolve_inventory_placement_anchor(
        &inventory_ui,
        &params.world,
        &grid_cells,
        &entry_widgets,
    ) {
        params.gate.block_gameplay_mouse = true;
        params.gate.spawn_handled_this_frame = true;
        match dev_place_item_at_anchor(
            &mut params.world,
            &ctx,
            inventory_id,
            item_id.clone(),
            quantity,
            anchor_x,
            anchor_y,
        ) {
            Ok(_index) => {
                params.dev_state.inventory.message = format!(
                    "Placed {} x{quantity} at ({anchor_x},{anchor_y})",
                    item_id.as_str()
                );
            }
            Err(err) => {
                params.dev_state.inventory.message = err.to_string();
            }
        }
        return;
    }

    if gameplay_inventory_ui_consumes_click(&inventory_ui, &grid_panes, &panel_roots) {
        params.gate.block_gameplay_mouse = true;
        params.gate.spawn_handled_this_frame = true;
        return;
    }

    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = params.config.chunk_layout();
    let vertical_scale = params
        .render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let Some(click) = terrain_click_to_world_position(&ray, &params.world, layout, vertical_scale)
    else {
        params.dev_state.inventory.message = "No terrain under cursor".into();
        return;
    };

    params.gate.block_gameplay_mouse = true;
    params.gate.spawn_handled_this_frame = true;
    let tick = params.simulation.current_tick;
    match dev_spawn_ground_pile(
        &mut params.world,
        item_id.clone(),
        quantity,
        click.world_position,
        tick,
    ) {
        Ok(pile_id) => {
            params.dev_state.inventory.message =
                format!("Spawned ground pile #{pile_id:?} x{quantity}");
        }
        Err(err) => {
            params.dev_state.inventory.message = err.to_string();
        }
    }
}

pub fn sync_dev_held_item_screen_ghost(
    dev_state: Res<DevModeState>,
    items: Res<ItemCatalog>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut commands: Commands,
    ghosts: Query<Entity, With<DevHeldItemGhostRoot>>,
) {
    for entity in &ghosts {
        commands.entity(entity).despawn();
    }
    let Some(item_id) = dev_state.dev_held_item_id() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let cursor = window.cursor_position().unwrap_or(Vec2::ZERO);
    let qty = dev_state
        .dev_held_item_id()
        .and_then(|id| items.get(&id))
        .map(|item| effective_held_placement_quantity(&dev_state, item))
        .unwrap_or_else(|| effective_held_quantity(&dev_state));
    let label = items
        .get(&item_id)
        .map(|item| format!("{} x{qty}", item.display_name))
        .unwrap_or_else(|| format!("{} x{qty}", item_id.as_str()));

    commands
        .spawn((
            DevHeldItemGhostRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x + 14.0),
                top: Val::Px(cursor.y + 14.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgba(0.95, 0.8, 0.2, 0.85)),
            BackgroundColor(Color::srgba(0.9, 0.75, 0.2, 0.45)),
            ZIndex(5000),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

pub fn sync_dev_held_item_world_ghost(
    dev_state: Res<DevModeState>,
    world: Res<WorldData>,
    world_config: Res<crate::world::WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    pile_settings: Res<ItemPilePresentationSettings>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<DevHeldItemWorldGhost>>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if dev_state.dev_held_item_id().is_none() {
        return;
    }
    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = world_config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let Some(click) = terrain_click_to_world_position(&ray, &world, layout, vertical_scale) else {
        return;
    };

    let mesh = meshes.add(Sphere::new(pile_settings.fallback_sphere_radius));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.9, 0.75, 0.2, 0.45),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let global = click.world_position.to_global(layout);
    commands.spawn((
        DevHeldItemWorldGhost,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(global + Vec3::Y * pile_settings.fallback_sphere_radius),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::dev_mode::{DefinitionId, DevModeState, DevTab};
    use crate::world::{
        ChunkCoord, InventoryProfileId, ItemDefinitionId, LocalPosition, WorldPosition,
        create_inventory, place_stack_first_fit,
    };
    use bevy::prelude::Vec3;

    fn test_ctx() -> (ItemCatalog, ItemCategoryCatalog, InventoryProfileCatalog) {
        (
            ItemCatalog::default(),
            ItemCategoryCatalog::default(),
            InventoryProfileCatalog::default(),
        )
    }

    #[test]
    fn catalog_item_selection_arms_held_item() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.active_tab = DevTab::Items;
        state.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));
        assert_eq!(
            state.dev_held_item_id(),
            Some(ItemDefinitionId::new("gold"))
        );
    }

    #[test]
    fn selecting_another_item_replaces_held_item() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.active_tab = DevTab::Items;
        state.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));
        state.select_definition(DefinitionId::Item(ItemDefinitionId::new("iron_ore")));
        assert_eq!(
            state.dev_held_item_id(),
            Some(ItemDefinitionId::new("iron_ore"))
        );
    }

    #[test]
    fn non_stackable_held_quantity_is_one_for_placement() {
        use crate::world::{ItemCategoryId, ItemDefinition};
        let crossbow = ItemDefinition::new(
            ItemDefinitionId::new("crossbow"),
            "Crossbow",
            "",
            ItemCategoryId::new("weapon"),
            2,
            3,
            false,
            1,
            5000,
            200,
            true,
        );
        let gold = ItemDefinition::new(
            ItemDefinitionId::new("gold"),
            "Gold",
            "",
            ItemCategoryId::new("currency"),
            1,
            1,
            true,
            999,
            1,
            1,
            true,
        );
        let mut state = DevModeState::default();
        state.inventory.quantity = 10;
        assert_eq!(effective_held_placement_quantity(&state, &crossbow), 1);
        assert_eq!(effective_held_placement_quantity(&state, &gold), 10);
    }

    #[test]
    fn quantity_change_affects_placement_quantity() {
        let mut state = DevModeState::default();
        state.inventory.quantity = 7;
        assert_eq!(effective_held_quantity(&state), 7);
        state.inventory.quantity_input = "42".into();
        state.text_focus = DevTextFieldFocus::ItemQuantity;
        assert_eq!(effective_held_quantity(&state), 42);
    }

    #[test]
    fn placement_does_not_clear_held_item() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.active_tab = DevTab::Items;
        state.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));
        let mut world = WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (_items, categories, profiles) = test_ctx();
        let ctx = InventoryCatalogCtx::new(&_items, &categories, &profiles);
        let inventory_id = create_inventory(
            world.inventory_store_mut(),
            &ctx,
            InventoryProfileId::new("unit_backpack_standard"),
            crate::world::InventoryOwnerRef::Detached,
        )
        .unwrap();
        super::super::ops::dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            5,
            0,
            0,
        )
        .unwrap();
        assert_eq!(
            state.dev_held_item_id(),
            Some(ItemDefinitionId::new("gold"))
        );
    }

    #[test]
    fn failed_placement_does_not_clear_held_item() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.active_tab = DevTab::Items;
        state.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));
        let mut world = WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (_items, categories, profiles) = test_ctx();
        let ctx = InventoryCatalogCtx::new(&_items, &categories, &profiles);
        let inventory_id = create_inventory(
            world.inventory_store_mut(),
            &ctx,
            InventoryProfileId::new("unit_backpack_standard"),
            crate::world::InventoryOwnerRef::Detached,
        )
        .unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            1,
        )
        .unwrap();
        let err = super::super::ops::dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            5,
            0,
            0,
        )
        .unwrap_err();
        assert!(!err.to_string().is_empty());
        assert_eq!(
            state.dev_held_item_id(),
            Some(ItemDefinitionId::new("gold"))
        );
    }

    #[test]
    fn right_click_clear_removes_held_item() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.active_tab = DevTab::Items;
        state.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));
        state.clear_dev_held_item();
        assert!(state.dev_held_item_id().is_none());
    }

    #[test]
    fn explicit_inventory_cell_placement_uses_anchor_api() {
        let mut world = WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (_items, categories, profiles) = test_ctx();
        let ctx = InventoryCatalogCtx::new(&_items, &categories, &profiles);
        let inventory_id = create_inventory(
            world.inventory_store_mut(),
            &ctx,
            InventoryProfileId::new("unit_backpack_standard"),
            crate::world::InventoryOwnerRef::Detached,
        )
        .unwrap();
        super::super::ops::dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            25,
            3,
            2,
        )
        .unwrap();
        let entry = world
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .placed_entries()
            .first()
            .unwrap();
        assert_eq!(entry.anchor_x, 3);
        assert_eq!(entry.anchor_y, 2);
    }

    #[test]
    fn terrain_placement_reaches_pile_authority() {
        let mut world = WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(4.0, 0.0, 4.0)),
        );
        let pile_id = super::super::ops::dev_spawn_ground_pile(
            &mut world,
            ItemDefinitionId::new("gold"),
            25,
            position,
            1,
        )
        .unwrap();
        assert_eq!(
            world
                .item_pile_store()
                .get(pile_id)
                .and_then(|p| p.stack_quantity()),
            Some(25)
        );
    }

    #[test]
    fn inventory_region_without_cell_blocks_terrain() {
        assert!(gameplay_inventory_ui_consumes_click_flags(
            true, true, false
        ));
    }

    #[test]
    fn inventory_closed_allows_terrain_path() {
        assert!(!gameplay_inventory_ui_consumes_click_flags(
            false, true, false
        ));
    }

    #[test]
    fn should_attempt_terrain_when_outside_inventory_ui() {
        assert!(!gameplay_inventory_ui_consumes_click_flags(
            true, false, false
        ));
    }

    #[test]
    fn panel_chrome_blocks_terrain_without_grid_cell() {
        assert!(gameplay_inventory_ui_consumes_click_flags(
            true, false, true
        ));
    }
}
