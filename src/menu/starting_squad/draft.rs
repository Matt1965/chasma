//! Transient starting-squad draft authority (CG8).

use bevy::prelude::*;

use crate::ui::unit_editor::UnitAppearanceDraft;
use crate::world::{
    AppearanceProfileCatalog, OriginCatalog, OriginDefinition, OriginId, UnitCatalog,
    UnitDefinitionId, resolve_canonical_default_appearance,
};

/// Stable draft-member identity within one origin squad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct SquadMemberDraftId(pub u32);

/// One configurable squad member before gameplay units exist.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SquadMemberDraft {
    pub id: SquadMemberDraftId,
    pub role_label: String,
    pub definition_id: UnitDefinitionId,
    pub preview_offset: Vec3,
    pub appearance: UnitAppearanceDraft,
    pub edited: bool,
}

/// Configured starting squad for one origin (client-local, not WorldData).
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
        let mut members = Vec::with_capacity(origin.roster.len());
        for (index, roster_member) in origin.roster.iter().enumerate() {
            let definition = unit_catalog
                .get(&roster_member.definition_id)
                .ok_or_else(|| {
                    format!(
                        "missing unit definition `{}` for origin `{}`",
                        roster_member.definition_id.as_str(),
                        origin.id.as_str()
                    )
                })?;
            let appearance = resolve_canonical_default_appearance(definition, appearance_profiles)
                .map_err(|error| {
                    format!(
                        "appearance default failed for `{}`: {error}",
                        roster_member.definition_id.as_str()
                    )
                })?;
            members.push(SquadMemberDraft {
                id: SquadMemberDraftId(index as u32),
                role_label: roster_member.role_label.clone(),
                definition_id: roster_member.definition_id.clone(),
                preview_offset: roster_member.preview_offset,
                appearance: UnitAppearanceDraft::from_live(
                    roster_member.definition_id.clone(),
                    Some(definition.display_name.clone()),
                    appearance,
                ),
                edited: false,
            });
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
