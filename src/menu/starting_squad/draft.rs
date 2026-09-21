//! Transient starting-squad draft authority (CG8).

use bevy::prelude::*;

use crate::ui::unit_editor::UnitAppearanceDraft;
use crate::world::{
    AppearanceProfileCatalog, InventorySubgraphSnapshot, OriginCatalog, OriginDefinition,
    OriginEquipmentSlotSnapshot, OriginId, UnitCatalog, UnitDefinitionId,
    resolve_canonical_default_appearance,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct SquadMemberDraftId(pub u32);

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SquadMemberDraft {
    pub id: SquadMemberDraftId,
    pub role_label: String,
    pub definition_id: UnitDefinitionId,
    pub preview_offset: Vec3,
    pub appearance: UnitAppearanceDraft,
    pub edited: bool,
    #[reflect(ignore)]
    pub personal_inventory: Option<InventorySubgraphSnapshot>,
    #[reflect(ignore)]
    pub equipment_slots: Vec<OriginEquipmentSlotSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct StartingSquadDraft {
    pub origin_id: OriginId,
    pub members: Vec<SquadMemberDraft>,
}

impl StartingSquadDraft {
    pub fn from_origin_definition(
        origin: &OriginDefinition,
        unit_catalog: &UnitCatalog,
        appearance_profiles: &AppearanceProfileCatalog,
    ) -> Result<Self, String> {
        let mut members = Vec::with_capacity(origin.members.len());
        for (index, member) in origin.members.iter().enumerate() {
            members.push(draft_member(member, index, unit_catalog, appearance_profiles)?);
        }
        Ok(Self {
            origin_id: origin.id.clone(),
            members,
        })
    }

    pub fn member_by_slot(&self, slot_index: usize) -> Option<&SquadMemberDraft> {
        self.members.get(slot_index)
    }

    pub fn member_by_slot_mut(&mut self, slot_index: usize) -> Option<&mut SquadMemberDraft> {
        self.members.get_mut(slot_index)
    }
}

fn draft_member(
    member: &crate::world::OriginSquadMemberSnapshot,
    index: usize,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
) -> Result<SquadMemberDraft, String> {
    let definition_id = member.definition_id.clone();
    let definition = unit_catalog
        .get(&definition_id)
        .ok_or_else(|| format!("missing unit definition `{}`", definition_id.as_str()))?;
    let appearance = if member.appearance.profile_id.is_empty() {
        resolve_canonical_default_appearance(definition, appearance_profiles)
            .map_err(|error| format!("appearance default failed: {error}"))?
    } else {
        member.appearance.to_unit_appearance()
    };
    Ok(SquadMemberDraft {
        id: SquadMemberDraftId(index as u32),
        role_label: member.role_label.clone(),
        definition_id,
        preview_offset: member.preview_offset_vec3(),
        appearance: UnitAppearanceDraft::from_live(
            member.definition_id.clone(),
            Some(definition.display_name.clone()),
            appearance,
        ),
        edited: false,
        personal_inventory: member.personal_inventory.clone(),
        equipment_slots: member.equipment_slots.clone(),
    })
}

pub fn build_starting_squad_draft(
    origin_id: &OriginId,
    origins: &OriginCatalog,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
) -> Result<StartingSquadDraft, String> {
    let origin = origins
        .get(origin_id)
        .ok_or_else(|| format!("unknown origin `{}`", origin_id.as_str()))?;
    StartingSquadDraft::from_origin_definition(origin, unit_catalog, appearance_profiles)
}
