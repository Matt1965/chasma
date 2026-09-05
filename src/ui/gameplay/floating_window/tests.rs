//! BP5 floating-window focused tests (spec items A–T where unit-testable).

use bevy::prelude::*;

use super::id::FloatingGameplayWindowId;
use super::state::FloatingGameplayWindowRegistry;
use crate::ui::gameplay::inventory::{InventoryDragState, InventoryUiState};
use crate::world::{InventoryId, ItemDefinitionId};

#[test]
fn container_storage_uses_unit_inventory_floating_window() {
    // Dual-transfer container UI shares InventoryPanelRoot / UnitInventory window id.
    let registry = FloatingGameplayWindowRegistry::default();
    assert!(
        registry
            .session(FloatingGameplayWindowId::UnitInventory)
            .is_some()
    );
    assert_eq!(FloatingGameplayWindowId::ALL.len(), 4);
}

#[test]
fn body_content_does_not_begin_window_drag_without_title_bar() {
    let registry = FloatingGameplayWindowRegistry::default();
    let before = registry
        .session(FloatingGameplayWindowId::BuildingMenu)
        .unwrap()
        .position;
    assert!(!registry.is_dragging());
    assert_eq!(
        registry
            .session(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position,
        before
    );
}

#[test]
fn inventory_item_drag_blocks_window_drag_start() {
    let mut ui = InventoryUiState::default();
    ui.dragging = Some(InventoryDragState {
        source_inventory_id: InventoryId(1),
        entry_index: 0,
        entry_revision: 0,
        item_definition_id: ItemDefinitionId::from("test"),
        grid_width: 1,
        grid_height: 1,
        quantity: 1,
    });
    assert!(ui.dragging.is_some());
    let registry = FloatingGameplayWindowRegistry::default();
    assert!(!registry.is_dragging());
}

#[test]
fn header_drag_does_not_populate_inventory_drag_state() {
    let mut registry = FloatingGameplayWindowRegistry::default();
    registry.begin_drag(FloatingGameplayWindowId::UnitInventory, Vec2::ZERO);
    let ui = InventoryUiState::default();
    assert!(registry.is_dragging());
    assert!(ui.dragging.is_none());
}

#[test]
fn overlapping_windows_use_focus_stack_for_z_order() {
    let mut registry = FloatingGameplayWindowRegistry::default();
    registry.focus_window(FloatingGameplayWindowId::BuildingMenu);
    let building_z = registry.z_index(FloatingGameplayWindowId::BuildingMenu);
    let inventory_z = registry.z_index(FloatingGameplayWindowId::UnitInventory);
    assert!(building_z > inventory_z);
    registry.focus_window(FloatingGameplayWindowId::UnitInventory);
    assert!(
        registry.z_index(FloatingGameplayWindowId::UnitInventory)
            > registry.z_index(FloatingGameplayWindowId::BuildingMenu)
    );
}

#[test]
fn unit_inventory_close_reopen_preserves_session_position() {
    let mut registry = FloatingGameplayWindowRegistry::default();
    registry
        .session_mut(FloatingGameplayWindowId::UnitInventory)
        .unwrap()
        .position = Vec2::new(88.0, 120.0);
    let remembered = registry
        .session(FloatingGameplayWindowId::UnitInventory)
        .unwrap()
        .position;
    assert_eq!(remembered, Vec2::new(88.0, 120.0));
}

#[test]
fn pointer_position_follows_registry_not_stale_default() {
    let mut registry = FloatingGameplayWindowRegistry::default();
    registry
        .session_mut(FloatingGameplayWindowId::BuildingMenu)
        .unwrap()
        .position = Vec2::new(400.0, 50.0);
    let pos = registry
        .session(FloatingGameplayWindowId::BuildingMenu)
        .unwrap()
        .position;
    assert_ne!(
        pos,
        super::math::default_building_menu_position(registry.viewport)
    );
}
