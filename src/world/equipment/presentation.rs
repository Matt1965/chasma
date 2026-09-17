//! Equipment visual presentation authoring (data-only, Slice 7).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::slot::EquipmentSlot;

/// How an equipped item is presented on a unit render rig.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum EquipmentPresentationMode {
    RigidAttachment,
    SkinnedOverlay,
}

impl EquipmentPresentationMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().replace(' ', "").as_str() {
            "rigidattachment" | "rigid" | "rigid_attachment" => Ok(Self::RigidAttachment),
            "skinnedoverlay" | "skinned" | "skinned_overlay" => Ok(Self::SkinnedOverlay),
            other => Err(format!("unknown equipment presentation mode `{other}`")),
        }
    }
}

/// Semantic attachment target on a unit render rig.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum EquipmentAttachmentSocket {
    RightHand,
    LeftHand,
    Head,
    Back,
    /// Stowed one-handed weapon (hip / pelvis semantic).
    RightHip,
}

impl EquipmentAttachmentSocket {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "righthand" | "right_hand" | "right-hand" => Ok(Self::RightHand),
            "lefthand" | "left_hand" | "left-hand" => Ok(Self::LeftHand),
            "head" => Ok(Self::Head),
            "back" => Ok(Self::Back),
            "righthip" | "right_hip" | "right-hip" | "weaponhip" | "weapon_hip" => {
                Ok(Self::RightHip)
            }
            other => Err(format!("unknown equipment attachment socket `{other}`")),
        }
    }
}

/// Optional per-item equipment visual placement.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct EquipmentPresentationAuthoring {
    pub socket: EquipmentAttachmentSocket,
    pub local_translation: Vec3,
    pub local_rotation: Quat,
    pub local_scale: Vec3,
}

impl Default for EquipmentPresentationAuthoring {
    fn default() -> Self {
        Self {
            socket: EquipmentAttachmentSocket::RightHand,
            local_translation: Vec3::ZERO,
            local_rotation: Quat::IDENTITY,
            local_scale: Vec3::ONE,
        }
    }
}

impl EquipmentPresentationAuthoring {
    pub fn with_socket(socket: EquipmentAttachmentSocket) -> Self {
        Self {
            socket,
            ..Default::default()
        }
    }
}

/// Equipment slots that may carry an equipped visual when a mapping exists.
pub fn slot_supports_equipment_presentation(slot: EquipmentSlot) -> bool {
    matches!(
        slot,
        EquipmentSlot::Weapon
            | EquipmentSlot::Offhand
            | EquipmentSlot::Head
            | EquipmentSlot::Body
            | EquipmentSlot::Arms
            | EquipmentSlot::Legs
            | EquipmentSlot::Feet
            | EquipmentSlot::Backpack
    )
}

/// Back-compat alias for rigid-only call sites during migration.
pub fn slot_supports_rigid_presentation(slot: EquipmentSlot) -> bool {
    slot_supports_equipment_presentation(slot)
}

/// Default semantic socket for a rigid equipment slot.
pub fn default_socket_for_slot(slot: EquipmentSlot) -> Option<EquipmentAttachmentSocket> {
    match slot {
        EquipmentSlot::Weapon => Some(EquipmentAttachmentSocket::RightHand),
        EquipmentSlot::Offhand => Some(EquipmentAttachmentSocket::LeftHand),
        EquipmentSlot::Head => Some(EquipmentAttachmentSocket::Head),
        EquipmentSlot::Backpack => Some(EquipmentAttachmentSocket::Back),
        _ => None,
    }
}
