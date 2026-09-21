//! Snapshot types for authored origin squad members.

use std::collections::BTreeMap;

use bevy::prelude::{Reflect, Vec3};
use serde::{Deserialize, Serialize};

use crate::world::inventory::{InventorySubgraphSnapshot, validate_inventory_subgraph};
use crate::world::{ItemCatalog, UnitAppearance, UnitDefinitionId};

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct OriginAppearanceSnapshot {
    pub profile_id: String,
    pub body_variant_id: String,
    pub height_scale: f32,
    #[serde(default)]
    pub morphs: BTreeMap<String, f32>,
    #[serde(default)]
    pub generation_seed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct OriginEquipmentSlotSnapshot {
    pub slot: String,
    #[reflect(ignore)]
    pub inventory: Option<InventorySubgraphSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct OriginSquadMemberSnapshot {
    pub role_label: String,
    pub definition_id: UnitDefinitionId,
    pub preview_offset_x: f32,
    pub preview_offset_y: f32,
    pub preview_offset_z: f32,
    pub appearance: OriginAppearanceSnapshot,
    #[serde(default)]
    #[reflect(ignore)]
    pub personal_inventory: Option<InventorySubgraphSnapshot>,
    #[serde(default)]
    #[reflect(ignore)]
    pub equipment_slots: Vec<OriginEquipmentSlotSnapshot>,
}

impl OriginSquadMemberSnapshot {
    pub fn preview_offset_vec3(&self) -> Vec3 {
        Vec3::new(self.preview_offset_x, self.preview_offset_y, self.preview_offset_z)
    }
}

impl OriginAppearanceSnapshot {
    pub fn from_unit_appearance(appearance: &UnitAppearance) -> Self {
        Self {
            profile_id: appearance.profile_id.as_str().to_string(),
            body_variant_id: appearance.body_variant_id.as_str().to_string(),
            height_scale: appearance.height_scale,
            morphs: appearance
                .morphs
                .iter()
                .map(|(key, value)| (key.as_str().to_string(), *value))
                .collect(),
            generation_seed: appearance.generation_seed,
        }
    }

    pub fn to_unit_appearance(&self) -> UnitAppearance {
        use crate::world::unit::appearance::{
            AppearanceParamId, AppearanceProfileId, BodyVariantId,
        };
        UnitAppearance {
            profile_id: AppearanceProfileId::new(&self.profile_id),
            body_variant_id: BodyVariantId::new(&self.body_variant_id),
            height_scale: self.height_scale,
            morphs: self
                .morphs
                .iter()
                .map(|(key, value)| (AppearanceParamId::new(key), *value))
                .collect(),
            generation_seed: self.generation_seed,
        }
    }
}

pub fn validate_member_inventory_snapshots(
    member: &OriginSquadMemberSnapshot,
    item_catalog: &ItemCatalog,
) -> Result<(), String> {
    let item_exists =
        |item_id: &str| item_catalog.get(&crate::world::ItemDefinitionId::new(item_id)).is_some();
    if let Some(inventory) = &member.personal_inventory {
        validate_inventory_subgraph(inventory, item_exists)
            .map_err(|error| format!("personal inventory: {error:?}"))?;
    }
    for slot in &member.equipment_slots {
        if let Some(inventory) = &slot.inventory {
            validate_inventory_subgraph(inventory, item_exists).map_err(|error| {
                format!("equipment slot `{}`: {error:?}", slot.slot)
            })?;
        }
    }
    Ok(())
}
