//! Client-local origin squad carousel and draft retention (CG8).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::{AppearanceProfileCatalog, OriginCatalog, OriginId, UnitCatalog};

use super::draft::{StartingSquadDraft, build_starting_squad_draft};

/// Full-roster view or focused member editing on the same preview stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum OriginSquadViewMode {
    #[default]
    FullSquad,
    FocusedMember {
        slot_index: usize,
    },
}

/// Active New Game character-generation session (client-local).
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
pub struct StartingSquadSession {
    pub selected_origin_index: usize,
    pub drafts_by_origin: HashMap<OriginId, StartingSquadDraft>,
    pub view_mode: OriginSquadViewMode,
    pub preview_yaw_radians: f32,
    pub preview_zoom: f32,
}

impl StartingSquadSession {
    pub fn new_for_first_origin(
        origins: &OriginCatalog,
        unit_catalog: &UnitCatalog,
        appearance_profiles: &AppearanceProfileCatalog,
    ) -> Result<Self, String> {
        let origin = origins
            .get_index(0)
            .ok_or_else(|| "origin catalog is empty".to_string())?;
        let draft = build_starting_squad_draft(
            &origin.id,
            origins,
            unit_catalog,
            appearance_profiles,
        )?;
        Ok(Self {
            selected_origin_index: 0,
            drafts_by_origin: HashMap::from([(origin.id.clone(), draft)]),
            view_mode: OriginSquadViewMode::FullSquad,
            preview_yaw_radians: 0.0,
            preview_zoom: 1.0,
        })
    }

    pub fn active_origin_id(&self, origins: &OriginCatalog) -> Option<OriginId> {
        origins
            .get_index(self.selected_origin_index)
            .map(|origin| origin.id.clone())
    }

    pub fn active_draft(&self, origins: &OriginCatalog) -> Option<&StartingSquadDraft> {
        let origin_id = self.active_origin_id(origins)?;
        self.drafts_by_origin.get(&origin_id)
    }

    pub fn active_draft_mut(
        &mut self,
        origins: &OriginCatalog,
    ) -> Option<&mut StartingSquadDraft> {
        let origin_id = self.active_origin_id(origins)?;
        self.drafts_by_origin.get_mut(&origin_id)
    }

    pub fn ensure_active_draft(
        &mut self,
        origins: &OriginCatalog,
        unit_catalog: &UnitCatalog,
        appearance_profiles: &AppearanceProfileCatalog,
    ) -> Result<(), String> {
        let origin_id = self
            .active_origin_id(origins)
            .ok_or_else(|| "no active origin".to_string())?;
        if self.drafts_by_origin.contains_key(&origin_id) {
            return Ok(());
        }
        let draft = build_starting_squad_draft(
            &origin_id,
            origins,
            unit_catalog,
            appearance_profiles,
        )?;
        self.drafts_by_origin.insert(origin_id, draft);
        Ok(())
    }

    pub fn cycle_origin(
        &mut self,
        delta: isize,
        origins: &OriginCatalog,
        unit_catalog: &UnitCatalog,
        appearance_profiles: &AppearanceProfileCatalog,
    ) -> Result<(), String> {
        let count = origins.definitions().len();
        if count == 0 {
            return Err("origin catalog is empty".to_string());
        }
        let next = (self.selected_origin_index as isize + delta).rem_euclid(count as isize) as usize;
        self.selected_origin_index = next;
        self.view_mode = OriginSquadViewMode::FullSquad;
        self.ensure_active_draft(origins, unit_catalog, appearance_profiles)?;
        Ok(())
    }

    pub fn enter_focus(&mut self, slot_index: usize) -> Result<(), String> {
        self.view_mode = OriginSquadViewMode::FocusedMember { slot_index };
        Ok(())
    }

    pub fn exit_focus(&mut self) {
        self.view_mode = OriginSquadViewMode::FullSquad;
    }

    pub fn is_focused(&self) -> bool {
        matches!(self.view_mode, OriginSquadViewMode::FocusedMember { .. })
    }

    pub fn focused_slot_index(&self) -> Option<usize> {
        match self.view_mode {
            OriginSquadViewMode::FullSquad => None,
            OriginSquadViewMode::FocusedMember { slot_index } => Some(slot_index),
        }
    }
}
