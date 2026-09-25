//! Authoritative equipment -> desired presentation reconciliation (pure).

use bevy::prelude::*;

use crate::world::equipment::{
    EquipmentAttachmentSocket, EquipmentPresentationAuthoring, EquipmentPresentationMode,
    EquipmentSlot, EquipmentVisualCatalog, default_socket_for_slot,
    effective_rigid_local_scale, effective_rigid_local_translation, skinned_fit_is_baked_offline,
    slot_supports_equipment_presentation,
};
use crate::world::{
    CorpseRecord, InventoryEntryContents, ItemCatalog, ItemDefinitionId, ItemInstanceId,
    ItemRenderKey, UnitCatalog, UnitId, UnitRecord, WorldData, effective_render_key_for_appearance,
    unit_in_active_combat,
};

/// Owner of an equipment presentation binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipmentPresentationOwner {
    Unit(UnitId),
    Corpse(crate::world::CorpseId),
}

/// One desired equipment visual binding for a rendered unit.
#[derive(Debug, Clone, PartialEq)]
pub struct DesiredEquipmentPresentation {
    pub unit_id: UnitId,
    pub slot: EquipmentSlot,
    pub item_instance_id: ItemInstanceId,
    pub item_definition_id: ItemDefinitionId,
    pub unit_render_key: String,
    pub render_key: ItemRenderKey,
    pub mode: EquipmentPresentationMode,
    pub presentation: EquipmentPresentationAuthoring,
}

/// Resolve equipped presentations for one unit using workbook visual mappings.
pub fn desired_equipment_presentations_for_unit(
    world: &WorldData,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
    unit_render_key: &str,
    unit: &UnitRecord,
) -> Vec<DesiredEquipmentPresentation> {
    let Some(equipment) = unit.equipment.as_ref() else {
        return Vec::new();
    };
    let mut desired = Vec::new();
    for slot in EquipmentSlot::ALL {
        if !slot_supports_equipment_presentation(slot) {
            continue;
        }
        let inventory_id = equipment.inventory_id(slot);
        let Some(inventory) = world.inventory_store().get(inventory_id) else {
            continue;
        };
        let Some(entry) = inventory.placed_entries().first() else {
            continue;
        };
        let item_instance_id = match &entry.contents {
            InventoryEntryContents::Unique { item_instance_id } => *item_instance_id,
            _ => continue,
        };
        let Some(instance) = world.item_instance_store().get(item_instance_id) else {
            continue;
        };
        if items.get(&instance.definition_id).is_none() {
            continue;
        }
        let Some(mapping) = visuals.resolve(&instance.definition_id, unit_render_key) else {
            continue;
        };
        let active_socket = mapping.socket;
        let combat_active = unit_in_active_combat(&unit.combat_state);
        let use_stowed = slot == EquipmentSlot::Weapon
            && !combat_active
            && mapping.stowed_socket.is_some();
        let (socket, local_translation, local_rotation, local_scale) = if use_stowed {
            (
                mapping.stowed_socket.unwrap_or(
                    active_socket.unwrap_or(EquipmentAttachmentSocket::RightHand),
                ),
                mapping.stowed_local_translation,
                mapping.stowed_local_rotation,
                mapping.stowed_local_scale,
            )
        } else {
            (
                active_socket.unwrap_or(
                    default_socket_for_slot(slot)
                        .unwrap_or(EquipmentAttachmentSocket::RightHand),
                ),
                mapping.local_translation,
                mapping.local_rotation,
                mapping.local_scale,
            )
        };
        let (local_translation, local_scale) = if skinned_fit_is_baked_offline(mapping.mode) {
            (local_translation, local_scale)
        } else {
            (
                effective_rigid_local_translation(local_translation, mapping.fit_offset),
                effective_rigid_local_scale(local_scale, mapping.fit_scale),
            )
        };
        let presentation = EquipmentPresentationAuthoring {
            socket,
            local_translation,
            local_rotation,
            local_scale,
        };
        desired.push(DesiredEquipmentPresentation {
            unit_id: unit.id,
            slot,
            item_instance_id,
            item_definition_id: instance.definition_id.clone(),
            unit_render_key: unit_render_key.to_string(),
            render_key: mapping.equipped_render_key.clone(),
            mode: mapping.mode,
            presentation,
        });
    }
    desired
}

/// Resolve equipped presentations for one corpse (always active socket — no combat stow).
pub fn desired_equipment_presentations_for_corpse(
    world: &WorldData,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
    unit_render_key: &str,
    corpse: &CorpseRecord,
) -> Vec<DesiredEquipmentPresentation> {
    let Some(equipment) = corpse.equipment.as_ref() else {
        return Vec::new();
    };
    let mut desired = Vec::new();
    for slot in EquipmentSlot::ALL {
        if !slot_supports_equipment_presentation(slot) {
            continue;
        }
        let inventory_id = equipment.inventory_id(slot);
        let Some(inventory) = world.inventory_store().get(inventory_id) else {
            continue;
        };
        let Some(entry) = inventory.placed_entries().first() else {
            continue;
        };
        let item_instance_id = match &entry.contents {
            InventoryEntryContents::Unique { item_instance_id } => *item_instance_id,
            _ => continue,
        };
        let Some(instance) = world.item_instance_store().get(item_instance_id) else {
            continue;
        };
        if items.get(&instance.definition_id).is_none() {
            continue;
        }
        let Some(mapping) = visuals.resolve(&instance.definition_id, unit_render_key) else {
            continue;
        };
        let socket = mapping
            .socket
            .unwrap_or(default_socket_for_slot(slot).unwrap_or(EquipmentAttachmentSocket::RightHand));
        let (local_translation, local_scale) = if skinned_fit_is_baked_offline(mapping.mode) {
            (mapping.local_translation, mapping.local_scale)
        } else {
            (
                effective_rigid_local_translation(mapping.local_translation, mapping.fit_offset),
                effective_rigid_local_scale(mapping.local_scale, mapping.fit_scale),
            )
        };
        let presentation = EquipmentPresentationAuthoring {
            socket,
            local_translation,
            local_rotation: mapping.local_rotation,
            local_scale,
        };
        desired.push(DesiredEquipmentPresentation {
            unit_id: corpse.origin_unit_id,
            slot,
            item_instance_id,
            item_definition_id: instance.definition_id.clone(),
            unit_render_key: unit_render_key.to_string(),
            render_key: mapping.equipped_render_key.clone(),
            mode: mapping.mode,
            presentation,
        });
    }
    desired
}

pub fn corpse_equipment_render_key(
    corpse: &CorpseRecord,
    definition: &crate::world::UnitDefinition,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
) -> Result<String, crate::world::AppearanceError> {
    if let Some(appearance) = corpse.appearance.as_ref() {
        return effective_render_key_for_appearance(appearance, appearance_profiles)
            .and_then(|key| {
                key.0.clone().ok_or(crate::world::AppearanceError::BodyVariantMissingRenderKey {
                    profile_id: appearance.profile_id.as_str().to_string(),
                    body_variant_id: appearance.body_variant_id.as_str().to_string(),
                })
            });
    }
    Ok(definition
        .render_key
        .0
        .clone()
        .unwrap_or_default())
}

/// Resolve all desired presentations for currently rendered units.
#[cfg_attr(not(test), allow(dead_code))] // batch resolver seam for future presentation pass
pub fn desired_equipment_presentations(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
    visible_unit_ids: &[UnitId],
) -> Vec<DesiredEquipmentPresentation> {
    let mut out = Vec::new();
    for unit_id in visible_unit_ids {
        let Some(unit) = world.get_unit(*unit_id) else {
            continue;
        };
        let Some(definition) = unit_catalog.get(&unit.definition_id) else {
            continue;
        };
        let unit_render_key = crate::world::effective_unit_render_key_str(
            unit,
            definition,
            appearance_profiles,
        )
        .unwrap_or_default();
        out.extend(desired_equipment_presentations_for_unit(
            world,
            items,
            visuals,
            &unit_render_key,
            unit,
        ));
    }
    out
}

/// Presentation key for reconciliation -- instance identity, not definition id.
#[cfg_attr(not(test), allow(dead_code))] // reconciliation key for future presentation pass
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EquipmentPresentationKey {
    pub unit_id: UnitId,
    pub slot: EquipmentSlot,
    pub item_instance_id: ItemInstanceId,
    pub equipped_render_key: String,
}

impl EquipmentPresentationKey {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_desired(value: &DesiredEquipmentPresentation) -> Self {
        Self {
            unit_id: value.unit_id,
            slot: value.slot,
            item_instance_id: value.item_instance_id,
            equipped_render_key: value.render_key.0.clone().unwrap_or_default(),
        }
    }
}
