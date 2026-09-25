//! ECS markers for equipment presentation entities.

use bevy::prelude::*;

use crate::world::equipment::EquipmentSlot;
use crate::world::AppearanceParamId;
use crate::world::{ItemInstanceId, UnitId};

/// Marker on a spawned equipment visual root (child of a unit socket bone).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitEquipmentVisual {
    pub unit_id: UnitId,
    pub slot: EquipmentSlot,
    pub item_instance_id: ItemInstanceId,
}

/// Marker on a spawned equipment visual root for a corpse render entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpseEquipmentVisual {
    pub corpse_id: crate::world::CorpseId,
    pub slot: EquipmentSlot,
    pub item_instance_id: ItemInstanceId,
}

/// glTF scene root for one equipment visual.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitEquipmentSceneRoot;

/// Armor scene spawned; joints still need rebinding to the live unit skeleton.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitEquipmentSkinnedPending;

/// Workbook-authored morph params consumed by one skinned equipment visual (CG5).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UnitEquipmentMorphConfig {
    pub consumed_morph_params: Vec<AppearanceParamId>,
}

/// Fingerprint of the last morph weights applied to one equipment visual.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct UnitEquipmentMorphFingerprint {
    pub profile_id: String,
    pub body_variant_id: String,
    pub morph_digest: u64,
    pub consumed_digest: u64,
}

impl UnitEquipmentMorphFingerprint {
    pub fn from_appearance(
        profile_id: &str,
        body_variant_id: &str,
        morph_digest: u64,
        consumed_digest: u64,
    ) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            body_variant_id: body_variant_id.to_string(),
            morph_digest,
            consumed_digest,
        }
    }
}
