//! Inventory drag ghost and world preview presentation (Slice 10).

use bevy::ecs::system::ParamSet;
use bevy::prelude::*;

use crate::item_piles::ItemPilePresentationSettings;
use crate::ui::gameplay::inventory::grid::{
    InventoryGridCell, InventoryGridPane, InventoryPaneSide,
};
use crate::ui::gameplay::inventory::preview::{
    INVENTORY_CELL_PX, InventoryDropTarget, InventoryPlacementPreview,
};
use crate::ui::gameplay::inventory::state::{InventoryDragPreviewState, InventoryUiState};
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::TEXT_PRIMARY;
use crate::world::{BuildingCatalog, BuildingInteractionProfileCatalog, WorldConfig, WorldData};

const GHOST_VALID: Color = Color::srgba(0.35, 0.65, 0.95, 0.55);
const GHOST_INVALID: Color = Color::srgba(0.95, 0.35, 0.30, 0.60);
const SOURCE_DRAGGED: Color = Color::srgba(0.25, 0.35, 0.55, 0.45);

#[derive(Component, Debug)]
pub struct InventoryDragGhost;

#[derive(Component, Debug)]
pub struct InventoryGroundPreview;

/// Track hovered grid cell / ground target while dragging.
pub fn update_inventory_drag_preview(
    world: Res<WorldData>,
    building_catalog: Res<BuildingCatalog>,
    interaction_catalog: Res<BuildingInteractionProfileCatalog>,
    ui: Res<InventoryUiState>,
    mut preview: ResMut<InventoryDragPreviewState>,
    mut interactions: ParamSet<(
        Query<(&Interaction, &InventoryGridCell)>,
        Query<&Interaction, (With<PlayerHudUi>, Without<InventoryGridCell>)>,
    )>,
) {
    let Some(drag) = ui.dragging.as_ref() else {
        if preview.placement != InventoryPlacementPreview::default() {
            *preview = InventoryDragPreviewState::default();
        }
        return;
    };

    let hovered_cell = interactions
        .p0()
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Hovered)
        .map(|(_, cell)| (cell.inventory_id, cell.x, cell.y));

    let hud_blocks_ground = interactions
        .p1()
        .iter()
        .any(|state| *state == Interaction::Hovered || *state == Interaction::Pressed);

    let target = if let Some((inventory_id, anchor_x, anchor_y)) = hovered_cell {
        InventoryDropTarget::GridCell {
            inventory_id,
            anchor_x,
            anchor_y,
        }
    } else if !hud_blocks_ground && ui.actor_unit_id.is_some() && ui.treasury_id.is_none() {
        InventoryDropTarget::GroundDrop
    } else {
        InventoryDropTarget::None
    };

    let placement = super::preview::evaluate_drop_target(
        world.as_ref(),
        building_catalog.as_ref(),
        interaction_catalog.as_ref(),
        ui.as_ref(),
        drag,
        target,
    );

    preview.placement = placement;
    if let Some(reason) = &preview.placement.reason {
        preview.status_line = reason.clone().message();
    } else if preview.placement.valid {
        preview.status_line.clear();
    } else {
        preview.status_line = "No valid drop target.".into();
    }
}

/// Spawn or update the inventory grid ghost overlay.
pub fn sync_inventory_drag_ghost(
    ui: Res<InventoryUiState>,
    preview: Res<InventoryDragPreviewState>,
    mut commands: Commands,
    panes: Query<(Entity, &InventoryGridPane)>,
    ghosts: Query<Entity, With<InventoryDragGhost>>,
) {
    for entity in &ghosts {
        commands.entity(entity).despawn();
    }

    let (
        Some(drag),
        InventoryDropTarget::GridCell {
            inventory_id,
            anchor_x,
            anchor_y,
        },
    ) = (ui.dragging.as_ref(), preview.placement.target)
    else {
        return;
    };

    let Some((pane_entity, _)) = panes
        .iter()
        .find(|(_, pane)| pane.inventory_id == inventory_id)
    else {
        return;
    };

    let color = if preview.placement.valid {
        GHOST_VALID
    } else {
        GHOST_INVALID
    };

    let ghost_w = INVENTORY_CELL_PX * f32::from(drag.grid_width) - 1.0;
    let ghost_h = INVENTORY_CELL_PX * f32::from(drag.grid_height) - 1.0;

    commands.entity(pane_entity).with_children(|grid| {
        grid.spawn((
            InventoryDragGhost,
            PlayerHudUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(f32::from(anchor_x) * INVENTORY_CELL_PX),
                top: Val::Px(f32::from(anchor_y) * INVENTORY_CELL_PX),
                width: Val::Px(ghost_w),
                height: Val::Px(ghost_h),
                ..default()
            },
            BackgroundColor(color),
            ZIndex(10),
        ))
        .with_children(|ghost| {
            ghost.spawn((
                Text::new(format!("{}×{}", drag.grid_width, drag.grid_height)),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));
        });
    });
}

/// Spawn or update the world-space ground-drop preview at the actor's feet.
pub fn sync_inventory_ground_preview(
    ui: Res<InventoryUiState>,
    preview: Res<InventoryDragPreviewState>,
    world: Res<WorldData>,
    world_config: Res<WorldConfig>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pile_settings: Res<ItemPilePresentationSettings>,
    existing: Query<Entity, With<InventoryGroundPreview>>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    if ui.dragging.is_none() || preview.placement.target != InventoryDropTarget::GroundDrop {
        return;
    }

    let Some(actor) = ui.actor_unit_id else {
        return;
    };
    let Some(unit) = world.get_unit(actor) else {
        return;
    };

    let mesh = meshes.add(Sphere::new(pile_settings.fallback_sphere_radius));
    let color = if preview.placement.valid {
        Color::srgba(0.35, 0.75, 0.45, 0.55)
    } else {
        Color::srgba(0.95, 0.30, 0.25, 0.60)
    };
    let material = materials.add(StandardMaterial {
        base_color: color,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    let layout = world_config.chunk_layout();
    let pos = unit.placement.position.to_global(layout);
    commands.spawn((
        InventoryGroundPreview,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(pos + Vec3::Y * pile_settings.fallback_sphere_radius),
        GlobalTransform::default(),
        Visibility::default(),
    ));
}

/// Remove preview entities when drag ends or panel closes.
pub fn cleanup_inventory_drag_previews(
    ui: Res<InventoryUiState>,
    mut preview: ResMut<InventoryDragPreviewState>,
    mut commands: Commands,
    ghosts: Query<Entity, With<InventoryDragGhost>>,
    ground: Query<Entity, With<InventoryGroundPreview>>,
) {
    if ui.dragging.is_some() {
        return;
    }
    for entity in ghosts.iter().chain(ground.iter()) {
        commands.entity(entity).despawn();
    }
    if preview.placement != InventoryPlacementPreview::default() || !preview.status_line.is_empty()
    {
        *preview = InventoryDragPreviewState::default();
    }
}

/// Apply dragged-source dimming when entries are rebuilt.
pub fn source_entry_drag_color(
    ui: &InventoryUiState,
    inventory_id: crate::world::InventoryId,
    entry_index: usize,
    default: Color,
) -> Color {
    if let Some(drag) = &ui.dragging {
        if drag.source_inventory_id == inventory_id && drag.entry_index == entry_index {
            return SOURCE_DRAGGED;
        }
    }
    default
}
