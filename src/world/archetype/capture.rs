//! Capture editor-authored archetype templates from live world instances.

use bevy::prelude::*;

use crate::world::building::BuildingRecord;
use crate::world::equipment::EquipmentSlot;
use crate::world::physical_gold_item_id;
use crate::world::inventory::InventoryEntryContents;
use crate::world::relationship::SpeciesId;
use crate::world::unit::UnitRecord;
use crate::world::{ItemCatalog, ItemDefinitionId, WorldData};

use super::building::{
    BuildingArchetypeCaptureMetadata, BuildingArchetypeDefinition, BuildingArchetypeId,
    BuildingArchetypeMember,
};
use super::durable_capture::capture_building_archetype_snapshot;
use super::building::DEFAULT_BUILDING_ARCHETYPE_CAPTURE_MARGIN_METERS;
use super::capture_volume::{
    BuildingArchetypeCaptureError, compute_building_archetype_capture_region,
    query_building_archetype_members,
};
use super::unit::{
    ArchetypeEquipmentEntry, ArchetypeInventoryStack, UnitArchetypeDefinition, UnitArchetypeId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchetypeCaptureError {
    UnitHasNoEquipment,
    MissingItemInstance,
    MissingItemDefinition(ItemDefinitionId),
}

/// Role-defining unit state captured from one configured instance.
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedUnitArchetypeTemplate {
    pub species_id: SpeciesId,
    pub affiliation_override: Option<crate::world::Affiliation>,
    pub equipment: Vec<ArchetypeEquipmentEntry>,
    pub inventory_stacks: Vec<ArchetypeInventoryStack>,
    pub gold_on_unit: u32,
}

pub fn capture_unit_archetype_template(
    world: &WorldData,
    unit: &UnitRecord,
    item_catalog: &ItemCatalog,
) -> Result<CapturedUnitArchetypeTemplate, ArchetypeCaptureError> {
    let equipment_inventories = unit
        .equipment
        .as_ref()
        .ok_or(ArchetypeCaptureError::UnitHasNoEquipment)?;

    let mut equipment = Vec::new();
    for slot in EquipmentSlot::ALL {
        let inventory_id = equipment_inventories.inventory_id(slot);
        let Some(record) = world.inventory_store().get(inventory_id) else {
            continue;
        };
        let Some(entry) = record.placed_entries().first() else {
            continue;
        };
        let item_id = resolve_entry_item_id(world, item_catalog, entry)?;
        equipment.push(ArchetypeEquipmentEntry { item_id, slot });
    }

    let mut inventory_stacks = Vec::new();
    if let Some(personal) = unit.inventory_id {
        if let Some(record) = world.inventory_store().get(personal) {
            let gold_item = physical_gold_item_id();
            for entry in record.placed_entries() {
                let item_id = resolve_entry_item_id(world, item_catalog, entry)?;
                if item_id.as_str() == gold_item.as_str() {
                    continue;
                }
                let quantity = match &entry.contents {
                    InventoryEntryContents::Stack { quantity, .. } => *quantity,
                    InventoryEntryContents::Unique { .. } => 1,
                };
                if quantity == 0 {
                    continue;
                }
                merge_inventory_stack(&mut inventory_stacks, item_id, quantity);
            }
        }
    }

    let gold_on_unit = unit_gold_count(world, unit);

    Ok(CapturedUnitArchetypeTemplate {
        species_id: unit.species_id.clone(),
        affiliation_override: Some(unit.affiliation),
        equipment,
        inventory_stacks,
        gold_on_unit,
    })
}

pub fn build_unit_archetype_definition(
    id: UnitArchetypeId,
    display_name: String,
    applicable_species: Vec<SpeciesId>,
    gold_min: u32,
    gold_max: u32,
    template: &CapturedUnitArchetypeTemplate,
    dialogue: Option<crate::world::dialogue::UnitDialogueConfig>,
) -> UnitArchetypeDefinition {
    UnitArchetypeDefinition {
        id,
        display_name,
        applicable_species,
        gold_min,
        gold_max,
        affiliation_override: template.affiliation_override,
        equipment: template.equipment.clone(),
        inventory_stacks: template.inventory_stacks.clone(),
        dialogue,
        enabled: true,
    }
}

pub fn build_building_archetype_definition(
    id: BuildingArchetypeId,
    display_name: String,
    building: &BuildingRecord,
    world: &WorldData,
    capture_metadata: BuildingArchetypeCaptureMetadata,
    members: Vec<BuildingArchetypeMember>,
    enabled: bool,
) -> BuildingArchetypeDefinition {
    BuildingArchetypeDefinition {
        id,
        display_name,
        base_building_id: building.definition_id.clone(),
        snapshot: capture_building_archetype_snapshot(world, building),
        capture_metadata,
        members,
        enabled,
    }
}

pub fn capture_building_archetype_members(
    world: &WorldData,
    root: &BuildingRecord,
    building_catalog: &crate::world::BuildingCatalog,
    footprint_catalog: &crate::world::FootprintCatalog,
    doodad_catalog: &crate::world::DoodadCatalog,
    capture_margin_meters: f32,
) -> Result<
    (
        BuildingArchetypeCaptureMetadata,
        Vec<BuildingArchetypeMember>,
    ),
    BuildingArchetypeCaptureError,
> {
    let region = compute_building_archetype_capture_region(
        world,
        root,
        building_catalog,
        footprint_catalog,
        capture_margin_meters,
    )?;
    let members = query_building_archetype_members(
        world,
        &region,
        root.id,
        building_catalog,
        doodad_catalog,
    );
    Ok((
        BuildingArchetypeCaptureMetadata {
            capture_margin_meters,
        },
        members,
    ))
}

pub fn default_building_archetype_capture_margin_meters() -> f32 {
    DEFAULT_BUILDING_ARCHETYPE_CAPTURE_MARGIN_METERS
}

fn resolve_entry_item_id(
    world: &WorldData,
    item_catalog: &ItemCatalog,
    entry: &crate::world::inventory::PlacedInventoryEntry,
) -> Result<ItemDefinitionId, ArchetypeCaptureError> {
    match &entry.contents {
        InventoryEntryContents::Unique { item_instance_id } => {
            let instance = world
                .item_instance_store()
                .get(*item_instance_id)
                .ok_or(ArchetypeCaptureError::MissingItemInstance)?;
            if item_catalog.get(&instance.definition_id).is_none() {
                return Err(ArchetypeCaptureError::MissingItemDefinition(
                    instance.definition_id.clone(),
                ));
            }
            Ok(instance.definition_id.clone())
        }
        InventoryEntryContents::Stack {
            item_definition_id, ..
        } => {
            if item_catalog.get(item_definition_id).is_none() {
                return Err(ArchetypeCaptureError::MissingItemDefinition(
                    item_definition_id.clone(),
                ));
            }
            Ok(item_definition_id.clone())
        }
    }
}

fn merge_inventory_stack(
    stacks: &mut Vec<ArchetypeInventoryStack>,
    item_id: ItemDefinitionId,
    quantity: u32,
) {
    if let Some(existing) = stacks.iter_mut().find(|stack| stack.item_id == item_id) {
        existing.quantity += quantity;
        return;
    }
    stacks.push(ArchetypeInventoryStack { item_id, quantity });
}

fn unit_gold_count(world: &WorldData, unit: &UnitRecord) -> u32 {
    let gold_item = physical_gold_item_id();
    let mut total = 0u32;
    if let Some(personal) = unit.inventory_id {
        if let Some(record) = world.inventory_store().get(personal) {
            for entry in record.placed_entries() {
                match &entry.contents {
                    InventoryEntryContents::Stack {
                        item_definition_id,
                        quantity,
                        ..
                    } if item_definition_id.as_str() == gold_item.as_str() => {
                        total += *quantity;
                    }
                    InventoryEntryContents::Unique { item_instance_id } => {
                        if let Some(instance) = world.item_instance_store().get(*item_instance_id) {
                            if instance.definition_id.as_str() == gold_item.as_str() {
                                total += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    total
}
