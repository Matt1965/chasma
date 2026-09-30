//! Appearance signatures for portrait cache identity and invalidation.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::world::equipment::{EquipmentSlot, slot_supports_equipment_presentation};
use crate::world::{
    AppearanceProfileCatalog, EquipmentVisualCatalog, InventoryEntryContents, ItemCatalog,
    UnitCatalog, UnitId, UnitRecord, WorldData, effective_render_key_for_appearance,
    effective_unit_render_key_str,
};

use super::cache::PortraitAppearanceSignature;

/// Equipment slots that can affect a head-and-shoulders portrait.
pub const PORTRAIT_EQUIPMENT_SLOTS: [EquipmentSlot; 3] = [
    EquipmentSlot::Head,
    EquipmentSlot::Body,
    EquipmentSlot::Arms,
];

/// Whether a slot change should invalidate the portrait cache.
pub fn portrait_slot_affects_signature(slot: EquipmentSlot) -> bool {
    PORTRAIT_EQUIPMENT_SLOTS.contains(&slot)
}

/// Build a stable signature from authoritative unit data (no simulation pose).
pub fn portrait_signature_for_unit(
    world: &WorldData,
    unit: &UnitRecord,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
) -> Option<PortraitAppearanceSignature> {
    let definition = unit_catalog.get(&unit.definition_id)?;
    let render_key =
        effective_unit_render_key_str(unit, definition, appearance_profiles).ok()?;
    let appearance = unit.appearance.as_ref();
    let mut hasher = DefaultHasher::new();
    unit.definition_id.as_str().hash(&mut hasher);
    render_key.hash(&mut hasher);
    if let Some(appearance) = appearance {
        appearance.profile_id.hash(&mut hasher);
        appearance.body_variant_id.hash(&mut hasher);
        appearance.height_scale.to_bits().hash(&mut hasher);
        if let Ok(key) = effective_render_key_for_appearance(appearance, appearance_profiles) {
            key.0.hash(&mut hasher);
        }
        for (name, value) in &appearance.morphs {
            name.hash(&mut hasher);
            value.to_bits().hash(&mut hasher);
        }
    }
    if let Some(equipment) = unit.equipment.as_ref() {
        for slot in EquipmentSlot::ALL {
            if !slot_supports_equipment_presentation(slot) || !portrait_slot_affects_signature(slot)
            {
                continue;
            }
            let inventory_id = equipment.inventory_id(slot);
            let Some(inventory) = world.inventory_store().get(inventory_id) else {
                slot.hash(&mut hasher);
                continue;
            };
            let entry = inventory.placed_entries().first();
            slot.hash(&mut hasher);
            match entry {
                None => 0u64.hash(&mut hasher),
                Some(entry) => {
                    let item_instance_id = match &entry.contents {
                        InventoryEntryContents::Unique { item_instance_id } => *item_instance_id,
                        _ => {
                            0u64.hash(&mut hasher);
                            continue;
                        }
                    };
                    item_instance_id.hash(&mut hasher);
                    if let Some(instance) = world.item_instance_store().get(item_instance_id) {
                        instance.definition_id.hash(&mut hasher);
                        if let Some(mapping) =
                            visuals.resolve(&instance.definition_id, &render_key)
                        {
                            mapping.socket.hash(&mut hasher);
                            mapping.mode.hash(&mut hasher);
                        }
                    }
                }
            }
        }
    }
    let digest = hasher.finish();
    Some(PortraitAppearanceSignature {
        unit_id: unit.id,
        digest,
    })
}

/// Convenience when only the digest is needed for comparisons.
pub fn portrait_digest_for_unit_id(
    world: &WorldData,
    unit_id: UnitId,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
) -> Option<u64> {
    let unit = world.get_unit(unit_id)?;
    portrait_signature_for_unit(
        world,
        unit,
        unit_catalog,
        appearance_profiles,
        items,
        visuals,
    )
    .map(|value| value.digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_slot_counts_for_signature() {
        assert!(portrait_slot_affects_signature(EquipmentSlot::Head));
        assert!(!portrait_slot_affects_signature(EquipmentSlot::Legs));
    }
}
