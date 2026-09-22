//! Equipment and equipped-backpack resolution for inventory UI (Slice 2).

use bevy::prelude::*;

use crate::ui::gameplay::floating_window::spawn_floating_section_well;
use crate::ui::gameplay::inventory::grid::{
    InventoryGridInteraction, InventoryGridPane, InventoryPaneKind, spawn_inventory_grid,
};
use crate::ui::gameplay::inventory::state::InventoryUiState;
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{TEXT_MUTED, TEXT_PRIMARY, panel_body_font};
use crate::world::equipment::{
    EquipmentSlot, UnitEquipmentInventories, resolve_equipped_backpack_internal,
};
use crate::world::{
    CorpseId, InventoryCatalogCtx, InventoryId, ItemCatalog, ItemInstanceStore, UnitId, WorldData,
};

/// Paper-doll slot positions as `(slot, column, row)` in a 3-column grid.
///
/// ```text
///        HEAD    BACKPACK
///  ARMS   BODY
///  WEAPON LEGS   OFFHAND
///         FEET
/// ```
pub const EQUIPMENT_LAYOUT: [(EquipmentSlot, u8, u8); 8] = [
    (EquipmentSlot::Head, 1, 0),
    (EquipmentSlot::Backpack, 2, 0),
    (EquipmentSlot::Arms, 0, 1),
    (EquipmentSlot::Body, 1, 1),
    (EquipmentSlot::Weapon, 0, 2),
    (EquipmentSlot::Legs, 1, 2),
    (EquipmentSlot::Offhand, 2, 2),
    (EquipmentSlot::Feet, 1, 3),
];

pub const EQUIPMENT_GRID_COLUMNS: u8 = 3;
pub const EQUIPMENT_GRID_ROWS: u8 = 4;

#[derive(Component, Debug)]
pub struct InventoryEquipmentSection;

#[derive(Component, Debug)]
pub struct InventoryBackpackSection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitEquipmentUiSnapshot {
    pub unit_id: UnitId,
    pub equipment: UnitEquipmentInventories,
    pub backpack_internal: Option<InventoryId>,
}

pub fn resolve_unit_equipment_ui(
    world: &WorldData,
    unit_id: UnitId,
) -> Option<UnitEquipmentUiSnapshot> {
    let unit = world.get_unit(unit_id)?;
    let equipment = unit.equipment?;
    let backpack_internal = resolve_equipped_backpack_internal(world, &equipment);
    Some(UnitEquipmentUiSnapshot {
        unit_id,
        equipment,
        backpack_internal,
    })
}

pub fn resolve_corpse_equipment_ui(
    world: &WorldData,
    corpse_id: CorpseId,
) -> Option<UnitEquipmentUiSnapshot> {
    let corpse = world.corpse_store().get(corpse_id)?;
    let equipment = corpse.equipment?;
    let backpack_internal = resolve_equipped_backpack_internal(world, &equipment);
    Some(UnitEquipmentUiSnapshot {
        unit_id: corpse.origin_unit_id,
        equipment,
        backpack_internal,
    })
}

pub fn equipment_revision(world: &WorldData, snapshot: &UnitEquipmentUiSnapshot) -> u64 {
    snapshot
        .equipment
        .all_inventory_ids()
        .iter()
        .map(|id| inventory_revision(world, *id))
        .sum()
}

pub fn backpack_revision(world: &WorldData, backpack_internal: Option<InventoryId>) -> u64 {
    backpack_internal
        .map(|id| inventory_revision(world, id))
        .unwrap_or(0)
}

fn inventory_revision(world: &WorldData, inventory_id: InventoryId) -> u64 {
    world
        .inventory_store()
        .get(inventory_id)
        .map(|r| r.placed_entries().len() as u64 * 10_000 + r.total_mass_grams())
        .unwrap_or(0)
}

pub fn slot_label(slot: EquipmentSlot) -> &'static str {
    match slot {
        EquipmentSlot::Head => "HEAD",
        EquipmentSlot::Body => "BODY",
        EquipmentSlot::Arms => "ARMS",
        EquipmentSlot::Legs => "LEGS",
        EquipmentSlot::Feet => "FEET",
        EquipmentSlot::Weapon => "WEAPON",
        EquipmentSlot::Offhand => "OFFHAND",
        EquipmentSlot::Backpack => "BACKPACK",
    }
}

pub fn spawn_equipment_section(
    parent: &mut ChildSpawnerCommands<'_>,
    world: &WorldData,
    _ctx: &InventoryCatalogCtx<'_>,
    items: &ItemCatalog,
    instance_store: &ItemInstanceStore,
    snapshot: &UnitEquipmentUiSnapshot,
    ui: &InventoryUiState,
    section_title: &str,
) {
    parent
        .spawn((
            InventoryEquipmentSection,
            Node {
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                row_gap: Val::Px(4.0),
                align_self: AlignSelf::FlexStart,
                ..default()
            },
        ))
        .with_children(|section| {
            section.spawn((
                Text::new(section_title),
                panel_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
            spawn_floating_section_well(section, |well| {
                well.spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: vec![
                        GridTrack::auto(),
                        GridTrack::auto(),
                        GridTrack::auto(),
                    ],
                    grid_template_rows: vec![
                        GridTrack::auto(),
                        GridTrack::auto(),
                        GridTrack::auto(),
                        GridTrack::auto(),
                    ],
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    justify_items: JustifyItems::Center,
                    ..default()
                })
                .with_children(|grid| {
                    for (slot, col, row) in EQUIPMENT_LAYOUT {
                        let inventory_id = snapshot.equipment.inventory_id(slot);
                        let Some(record) = world.inventory_store().get(inventory_id) else {
                            continue;
                        };
                        grid.spawn(Node {
                            grid_column: GridPlacement::start(i16::from(col + 1)),
                            grid_row: GridPlacement::start(i16::from(row + 1)),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(2.0),
                            ..default()
                        })
                        .with_children(|slot_root| {
                            slot_root.spawn((
                                Text::new(slot_label(slot)),
                                TextFont {
                                    font_size: 9.0,
                                    ..default()
                                },
                                TextColor(TEXT_MUTED),
                            ));
                            slot_root
                                .spawn((
                                    InventoryGridPane {
                                        inventory_id,
                                        pane_kind: InventoryPaneKind::Equipment(slot),
                                    },
                                    PlayerHudUi,
                                    Button,
                                    Interaction::None,
                                    Node {
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                ))
                                .with_children(|pane| {
                                    spawn_inventory_grid(
                                        pane,
                                        record,
                                        inventory_id,
                                        items,
                                        instance_store,
                                        InventoryGridInteraction::Interactive {
                                            pane_kind: InventoryPaneKind::Equipment(slot),
                                        },
                                        Some(ui),
                                    );
                                });
                        });
                    }
                });
            });
        });
}

pub fn spawn_backpack_internal_section(
    parent: &mut ChildSpawnerCommands<'_>,
    world: &WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    items: &ItemCatalog,
    instance_store: &ItemInstanceStore,
    internal_id: InventoryId,
    ui: &InventoryUiState,
    section_title: &str,
) {
    let Some(record) = world.inventory_store().get(internal_id) else {
        return;
    };
    let weight = crate::world::query_inventory_weight(record, ctx)
        .map(|w| format!("{:.1} kg", w.total_mass_grams as f64 / 1000.0))
        .unwrap_or_else(|_| "Weight unavailable".into());

    parent
        .spawn((
            InventoryBackpackSection,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|section| {
            section.spawn((
                Text::new(format!("{section_title}\n{weight}")),
                panel_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
            spawn_floating_section_well(section, |well| {
                well.spawn((
                    InventoryGridPane {
                        inventory_id: internal_id,
                        pane_kind: InventoryPaneKind::BackpackInternal,
                    },
                    PlayerHudUi,
                    Button,
                    Interaction::None,
                    Node {
                        flex_shrink: 0.0,
                        ..default()
                    },
                ))
                .with_children(|pane| {
                    spawn_inventory_grid(
                        pane,
                        record,
                        internal_id,
                        items,
                        instance_store,
                        InventoryGridInteraction::Interactive {
                            pane_kind: InventoryPaneKind::BackpackInternal,
                        },
                        Some(ui),
                    );
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::equipment::EquipmentSlot;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, InventoryProfileCatalog,
        InventoryProfileId, ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId,
        ItemDefinition, ItemDefinitionId, LocalPosition, TransferPlacementPolicy, UnitCatalog,
        UnitDefinitionId, UnitOwnership, UnitSource, WorldPosition, create_item_instance,
        create_unit_with_inventory, place_unique_first_fit, starter_inventory_profile_definitions,
        starter_unit_definitions, transfer_unique_item,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn test_ctx() -> &'static InventoryCatalogCtx<'static> {
        static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
        CTX.get_or_init(|| {
            let categories = Box::leak(Box::new(
                ItemCategoryCatalog::from_definitions(vec![ItemCategoryDefinition::new(
                    ItemCategoryId::new("container"),
                    "Container",
                    "",
                    true,
                )])
                .unwrap(),
            ));
            let items = Box::leak(Box::new(
                ItemCatalog::from_definitions(
                    vec![
                        ItemDefinition::new(
                            ItemDefinitionId::new("leather_backpack"),
                            "Basic Backpack",
                            "",
                            ItemCategoryId::new("container"),
                            2,
                            3,
                            false,
                            1,
                            500,
                            1,
                            true,
                        )
                        .with_unique_instance_required(true)
                        .with_equipment_slots(vec![EquipmentSlot::Backpack])
                        .with_backpack_profile_id(InventoryProfileId::new(
                            "backpack_basic_internal",
                        )),
                    ],
                    categories,
                )
                .unwrap(),
            ));
            let profiles = Box::leak(Box::new(
                InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                    .unwrap(),
            ));
            InventoryCatalogCtx::new(items, categories, profiles)
        })
    }

    #[test]
    fn equipment_layout_places_all_slots_in_compact_grid() {
        assert_eq!(EQUIPMENT_LAYOUT.len(), 8);
        assert_eq!(EQUIPMENT_GRID_COLUMNS, 3);
        assert_eq!(EQUIPMENT_GRID_ROWS, 4);
        let positions = EQUIPMENT_LAYOUT
            .iter()
            .map(|(slot, col, row)| {
                assert!(*col < EQUIPMENT_GRID_COLUMNS);
                assert!(*row < EQUIPMENT_GRID_ROWS);
                (col, row, slot)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            positions.len(),
            positions
                .iter()
                .map(|(col, row, _)| (*col, *row))
                .collect::<std::collections::HashSet<_>>()
                .len(),
            "equipment slots must not overlap in the paper-doll grid"
        );
    }

    #[test]
    fn equipment_ui_resolves_all_eight_slot_inventories() {
        let mut world = flat_world();
        let ctx = test_ctx();
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let unit = create_unit_with_inventory(
            &catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 1.0),
            UnitSource::Dev,
            UnitOwnership::hostile(),
            &ctx,
        )
        .unwrap();
        let snapshot = resolve_unit_equipment_ui(&world, unit.id).unwrap();
        for slot in EquipmentSlot::ALL {
            assert_eq!(
                snapshot.equipment.inventory_id(slot),
                unit.equipment.unwrap().inventory_id(slot)
            );
        }
        assert!(snapshot.backpack_internal.is_none());
    }

    #[test]
    fn backpack_pane_resolves_contained_inventory_when_equipped() {
        let mut world = flat_world();
        let ctx = test_ctx();
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let unit = create_unit_with_inventory(
            &catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 1.0),
            UnitSource::Dev,
            UnitOwnership::hostile(),
            &ctx,
        )
        .unwrap();
        let personal = unit.inventory_id.unwrap();
        let backpack_slot = unit.equipment.unwrap().backpack;
        let backpack_id = {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            create_item_instance(
                inventory_store,
                instance_store,
                &ctx,
                ItemDefinitionId::new("leather_backpack"),
                Default::default(),
            )
            .unwrap()
        };
        let expected_internal = world
            .item_instance_store()
            .get(backpack_id)
            .unwrap()
            .contained_inventory_id
            .unwrap();
        {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            place_unique_first_fit(inventory_store, instance_store, &ctx, personal, backpack_id)
                .unwrap();
            transfer_unique_item(
                inventory_store,
                instance_store,
                &ctx,
                personal,
                0,
                backpack_id,
                backpack_slot,
                TransferPlacementPolicy::ExactCell { x: 0, y: 0 },
            )
            .unwrap();
        }
        let snapshot = resolve_unit_equipment_ui(&world, unit.id).unwrap();
        assert_eq!(snapshot.backpack_internal, Some(expected_internal));
    }
}
