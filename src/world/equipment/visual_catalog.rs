//! Workbook-authoritative equipment visual variant mappings (Slice 7.1).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::{AppearanceParamId, ItemDefinitionId, ItemRenderKey};

use super::presentation::{EquipmentAttachmentSocket, EquipmentPresentationMode};

/// One visual variant for an equipped item on a specific unit render profile.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct EquipmentVisualMapping {
    pub item_id: ItemDefinitionId,
    pub unit_render_key: String,
    pub equipped_render_key: ItemRenderKey,
    pub mode: EquipmentPresentationMode,
    pub socket: Option<EquipmentAttachmentSocket>,
    pub local_translation: Vec3,
    pub local_rotation: Quat,
    pub local_scale: Vec3,
    /// Optional stowed presentation when not in active combat (Slice 8).
    pub stowed_socket: Option<EquipmentAttachmentSocket>,
    pub stowed_local_translation: Vec3,
    pub stowed_local_rotation: Quat,
    pub stowed_local_scale: Vec3,
    /// Uniform bind-pose inflation baked into skinned GLBs (default 1.0).
    pub fit_scale: f32,
    /// Bind-pose translation baked into skinned GLBs (default zero).
    pub fit_offset: Vec3,
    /// Semantic appearance parameters this visual consumes for morph deformation (CG5).
    pub consumed_morph_params: Vec<AppearanceParamId>,
}

/// Catalog of equipment visual mappings imported from the design workbook.
#[derive(Debug, Clone, Default, Resource, Reflect)]
pub struct EquipmentVisualCatalog {
    mappings: Vec<EquipmentVisualMapping>,
    by_item_and_profile: HashMap<(ItemDefinitionId, String), usize>,
}

impl EquipmentVisualCatalog {
    pub fn from_mappings(mappings: Vec<EquipmentVisualMapping>) -> Result<Self, String> {
        let mut by_item_and_profile = HashMap::new();
        for (index, mapping) in mappings.iter().enumerate() {
            let key = (mapping.item_id.clone(), mapping.unit_render_key.clone());
            if by_item_and_profile.insert(key, index).is_some() {
                return Err(format!(
                    "duplicate equipment visual mapping for item `{}` on unit render key `{}`",
                    mapping.item_id.as_str(),
                    mapping.unit_render_key
                ));
            }
        }
        Ok(Self {
            mappings,
            by_item_and_profile,
        })
    }

    pub fn mappings(&self) -> &[EquipmentVisualMapping] {
        &self.mappings
    }

    pub fn resolve(
        &self,
        item_id: &ItemDefinitionId,
        unit_render_key: &str,
    ) -> Option<&EquipmentVisualMapping> {
        self.by_item_and_profile
            .get(&(item_id.clone(), unit_render_key.to_string()))
            .map(|index| &self.mappings[*index])
    }
}
