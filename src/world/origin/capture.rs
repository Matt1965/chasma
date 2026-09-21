use bevy::prelude::Vec3;

use crate::world::equipment::EquipmentSlot;
use crate::world::inventory::capture_inventory_subgraph;
use crate::world::{UnitId, WorldData};

use super::snapshot::{
    OriginAppearanceSnapshot, OriginEquipmentSlotSnapshot, OriginSquadMemberSnapshot,
};

pub fn capture_origin_member_from_unit(
    world: &WorldData,
    unit_id: UnitId,
    role_label: String,
    preview_offset: Vec3,
) -> Option<OriginSquadMemberSnapshot> {
    let record = world.get_unit(unit_id)?;
    let appearance = record
        .appearance
        .as_ref()
        .map(OriginAppearanceSnapshot::from_unit_appearance);
    let (personal_inventory, equipment_slots) = capture_member_inventory_loadout(world, unit_id);
    Some(OriginSquadMemberSnapshot {
        role_label,
        definition_id: record.definition_id.clone(),
        preview_offset_x: preview_offset.x,
        preview_offset_y: preview_offset.y,
        preview_offset_z: preview_offset.z,
        appearance: appearance.unwrap_or_else(|| OriginAppearanceSnapshot {
            profile_id: String::new(),
            body_variant_id: String::new(),
            height_scale: 1.0,
            morphs: Default::default(),
            generation_seed: None,
        }),
        personal_inventory,
        equipment_slots,
    })
}

pub fn capture_member_inventory_loadout(
    world: &WorldData,
    unit_id: UnitId,
) -> (
    Option<crate::world::InventorySubgraphSnapshot>,
    Vec<OriginEquipmentSlotSnapshot>,
) {
    let record = world.get_unit(unit_id);
    let personal = record
        .and_then(|value| value.inventory_id)
        .and_then(|id| has_contents(world, id).then(|| capture_inventory_subgraph(world, id)).flatten());
    let equipment_slots = record
        .and_then(|value| value.equipment.as_ref())
        .map(|equipment| {
            EquipmentSlot::ALL
                .iter()
                .filter_map(|slot| {
                    let id = equipment.inventory_id(*slot);
                    if !has_contents(world, id) {
                        return None;
                    }
                    capture_inventory_subgraph(world, id).map(|inventory| {
                        OriginEquipmentSlotSnapshot {
                            slot: slot.display_name().to_ascii_lowercase(),
                            inventory: Some(inventory),
                        }
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (personal, equipment_slots)
}

fn has_contents(world: &WorldData, inventory_id: crate::world::InventoryId) -> bool {
    world
        .inventory_store()
        .get(inventory_id)
        .is_some_and(|record| !record.placed_entries().is_empty())
}
