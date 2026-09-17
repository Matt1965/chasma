//! Equipment visual variant import schema (Slice 7.1).

use bevy::prelude::*;

use crate::world::equipment::{
    EquipmentAttachmentSocket, EquipmentPresentationMode, EquipmentVisualMapping,
};
use crate::world::{ItemDefinitionId, ItemRenderKey};

pub const REQUIRED_COLUMNS: &[&str] = &[
    "Item ID",
    "Unit Render Key",
    "Equipped Render Key",
    "Presentation Mode",
];

pub const OPTIONAL_COLUMNS: &[&str] = &[
    "Socket",
    "Local Translation",
    "Local Rotation",
    "Local Scale",
    "Stowed Socket",
    "Stowed Local Translation",
    "Stowed Local Rotation",
    "Stowed Local Scale",
    "Consumed Morph Params",
];

#[derive(Debug, Clone, PartialEq)]
pub struct EquipmentVisualImportRow {
    pub row_number: usize,
    pub item_id: String,
    pub unit_render_key: String,
    pub equipped_render_key: String,
    pub presentation_mode: String,
    pub socket: Option<String>,
    pub local_translation: Option<String>,
    pub local_rotation: Option<String>,
    pub local_scale: Option<String>,
    pub stowed_socket: Option<String>,
    pub stowed_local_translation: Option<String>,
    pub stowed_local_rotation: Option<String>,
    pub stowed_local_scale: Option<String>,
    pub consumed_morph_params: Option<String>,
}

impl EquipmentVisualImportRow {
    pub fn to_mapping(&self) -> Result<EquipmentVisualMapping, String> {
        let mode = EquipmentPresentationMode::parse(&self.presentation_mode)?;
        let socket = self
            .socket
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| EquipmentAttachmentSocket::parse(value))
            .transpose()?;
        if mode == EquipmentPresentationMode::RigidAttachment && socket.is_none() {
            return Err(format!(
                "row {}: rigid attachment requires Socket",
                self.row_number
            ));
        }
        let stowed_socket = self
            .stowed_socket
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| EquipmentAttachmentSocket::parse(value))
            .transpose()?;
        Ok(EquipmentVisualMapping {
            item_id: ItemDefinitionId::new(self.item_id.trim()),
            unit_render_key: self.unit_render_key.trim().to_string(),
            equipped_render_key: ItemRenderKey::reserved(self.equipped_render_key.trim()),
            mode,
            socket,
            local_translation: parse_vec3(self.local_translation.as_deref(), Vec3::ZERO)?,
            local_rotation: parse_quat(self.local_rotation.as_deref(), Quat::IDENTITY)?,
            local_scale: parse_vec3(self.local_scale.as_deref(), Vec3::ONE)?,
            stowed_socket,
            stowed_local_translation: parse_vec3(
                self.stowed_local_translation.as_deref(),
                Vec3::ZERO,
            )?,
            stowed_local_rotation: parse_quat(
                self.stowed_local_rotation.as_deref(),
                Quat::IDENTITY,
            )?,
            stowed_local_scale: parse_vec3(self.stowed_local_scale.as_deref(), Vec3::ONE)?,
            consumed_morph_params: parse_consumed_morph_params(
                self.consumed_morph_params.as_deref(),
                self.row_number,
            )?,
        })
    }
}

fn parse_consumed_morph_params(
    raw: Option<&str>,
    row_number: usize,
) -> Result<Vec<crate::world::AppearanceParamId>, String> {
    use std::collections::HashSet;

    use crate::world::AppearanceParamId;

    let raw = raw.map(str::trim).filter(|value| !value.is_empty());
    match raw {
        None => Ok(Vec::new()),
        Some(value) => {
            let mut params = Vec::new();
            let mut seen = HashSet::new();
            for part in value.split(',') {
                let id = part.trim();
                if id.is_empty() {
                    continue;
                }
                let param = AppearanceParamId::new(id);
                if !seen.insert(param.clone()) {
                    return Err(format!(
                        "row {}: duplicate consumed morph param `{}`",
                        row_number,
                        id
                    ));
                }
                params.push(param);
            }
            Ok(params)
        }
    }
}

fn parse_vec3(raw: Option<&str>, default: Vec3) -> Result<Vec3, String> {
    let raw = raw.map(str::trim).filter(|value| !value.is_empty());
    match raw {
        None => Ok(default),
        Some(value) => {
            let parts: Vec<&str> = value.split(',').map(str::trim).collect();
            if parts.len() != 3 {
                return Err(format!("expected vec3 `x,y,z`, got `{value}`"));
            }
            Ok(Vec3::new(
                parts[0]
                    .parse()
                    .map_err(|_| format!("invalid x in `{value}`"))?,
                parts[1]
                    .parse()
                    .map_err(|_| format!("invalid y in `{value}`"))?,
                parts[2]
                    .parse()
                    .map_err(|_| format!("invalid z in `{value}`"))?,
            ))
        }
    }
}

fn parse_quat(raw: Option<&str>, default: Quat) -> Result<Quat, String> {
    let raw = raw.map(str::trim).filter(|value| !value.is_empty());
    match raw {
        None => Ok(default),
        Some(value) => {
            let parts: Vec<&str> = value.split(',').map(str::trim).collect();
            if parts.len() != 4 {
                return Err(format!("expected quat `x,y,z,w`, got `{value}`"));
            }
            Ok(Quat::from_xyzw(
                parts[0]
                    .parse()
                    .map_err(|_| format!("invalid x in `{value}`"))?,
                parts[1]
                    .parse()
                    .map_err(|_| format!("invalid y in `{value}`"))?,
                parts[2]
                    .parse()
                    .map_err(|_| format!("invalid z in `{value}`"))?,
                parts[3]
                    .parse()
                    .map_err(|_| format!("invalid w in `{value}`"))?,
            ))
        }
    }
}
