//! Unified client-local world selection (Dev UI Revamp Slice 1).
//!
//! [`WorldSelectionState`] is the single authority for which world object category is selected.
//! [`SelectedUnits`] remains the storage for the unit id set; all writes go through
//! [`apply_world_selection`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::ui::gameplay::{GameplayBuildingSelection, PlayerHudState, sync_primary_selection};
use crate::units::input::SelectedUnits;
use crate::world::{BuildingId, DoodadId, ItemPileId, UnitId, WorldData};

/// Monotonic counter bumped on every [`apply_world_selection`] call.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorldSelectionRevision(pub u64);

/// Active world-selection category. Only one category at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum WorldSelectionCategory {
    #[default]
    None,
    Units,
    Building,
    Doodad,
    ItemPile,
}

/// Client-local authoritative world selection (not simulation truth).
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldSelectionState {
    pub category: WorldSelectionCategory,
    pub building_id: Option<BuildingId>,
    pub doodad_id: Option<DoodadId>,
    pub pile_id: Option<ItemPileId>,
}

impl WorldSelectionState {
    pub fn has_transform_target(&self) -> bool {
        matches!(
            self.category,
            WorldSelectionCategory::Building | WorldSelectionCategory::Doodad
        )
    }

    pub fn transform_doodad(&self) -> Option<DoodadId> {
        match self.category {
            WorldSelectionCategory::Doodad => self.doodad_id,
            _ => None,
        }
    }

    pub fn transform_building(&self) -> Option<BuildingId> {
        match self.category {
            WorldSelectionCategory::Building => self.building_id,
            _ => None,
        }
    }

    pub fn primary_unit<'a>(&self, selected_units: &'a SelectedUnits) -> Option<UnitId> {
        if self.category != WorldSelectionCategory::Units {
            return None;
        }
        crate::ui::gameplay::primary_selected_unit(selected_units)
    }
}

/// Selection mutations applied through one code path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldSelectionChange {
    SelectUnit {
        unit_id: UnitId,
    },
    ToggleUnit {
        unit_id: UnitId,
    },
    ReplaceUnits {
        unit_ids: Vec<UnitId>,
    },
    AddUnits {
        unit_ids: Vec<UnitId>,
    },
    SelectBuilding {
        building_id: BuildingId,
    },
    SelectDoodad {
        doodad_id: DoodadId,
    },
    SelectItemPile {
        pile_id: ItemPileId,
    },
    /// Clears building, doodad, and pile selection. Unit set unchanged.
    ClearWorldObject,
    /// Clears all categories including units.
    ClearAll,
}

/// Bevy system param bundle for selection writes (keeps systems under the 16-param limit).
#[derive(SystemParam)]
pub struct WorldSelectionWriteParams<'w> {
    pub world_selection: ResMut<'w, WorldSelectionState>,
    pub selection_revision: ResMut<'w, WorldSelectionRevision>,
    pub selected_units: ResMut<'w, SelectedUnits>,
    pub building_selection: ResMut<'w, GameplayBuildingSelection>,
}

impl WorldSelectionWriteParams<'_> {
    pub fn apply<'a>(
        &'a mut self,
        hud: Option<&'a mut PlayerHudState>,
    ) -> ApplyWorldSelectionParams<'a> {
        ApplyWorldSelectionParams {
            world_selection: &mut self.world_selection,
            selected_units: &mut self.selected_units,
            building_selection: &mut self.building_selection,
            hud,
            revision: Some(&mut self.selection_revision),
        }
    }
}

/// Bundled mutable selection targets for [`apply_world_selection`].
pub struct ApplyWorldSelectionParams<'a> {
    pub world_selection: &'a mut WorldSelectionState,
    pub selected_units: &'a mut SelectedUnits,
    pub building_selection: &'a mut GameplayBuildingSelection,
    pub hud: Option<&'a mut PlayerHudState>,
    pub revision: Option<&'a mut WorldSelectionRevision>,
}

/// Apply one selection change and invalidate derived inspector caches.
pub fn apply_world_selection(
    change: WorldSelectionChange,
    params: &mut ApplyWorldSelectionParams<'_>,
) {
    match change {
        WorldSelectionChange::SelectUnit { unit_id } => {
            clear_world_object_fields(params);
            params.selected_units.set_single(unit_id);
            params.world_selection.category = WorldSelectionCategory::Units;
        }
        WorldSelectionChange::ToggleUnit { unit_id } => {
            clear_world_object_fields(params);
            params.selected_units.toggle(unit_id);
            params.world_selection.category = if params.selected_units.is_empty() {
                WorldSelectionCategory::None
            } else {
                WorldSelectionCategory::Units
            };
        }
        WorldSelectionChange::ReplaceUnits { unit_ids } => {
            clear_world_object_fields(params);
            params.selected_units.replace_with(unit_ids);
            params.world_selection.category = if params.selected_units.is_empty() {
                WorldSelectionCategory::None
            } else {
                WorldSelectionCategory::Units
            };
        }
        WorldSelectionChange::AddUnits { unit_ids } => {
            clear_world_object_fields(params);
            params.selected_units.add_all(unit_ids);
            params.world_selection.category = if params.selected_units.is_empty() {
                WorldSelectionCategory::None
            } else {
                WorldSelectionCategory::Units
            };
        }
        WorldSelectionChange::SelectBuilding { building_id } => {
            clear_unit_fields(params);
            clear_world_object_fields(params);
            params.world_selection.category = WorldSelectionCategory::Building;
            params.world_selection.building_id = Some(building_id);
            params.building_selection.set(Some(building_id));
        }
        WorldSelectionChange::SelectDoodad { doodad_id } => {
            clear_unit_fields(params);
            clear_world_object_fields(params);
            params.world_selection.category = WorldSelectionCategory::Doodad;
            params.world_selection.doodad_id = Some(doodad_id);
            params.building_selection.set(None);
        }
        WorldSelectionChange::SelectItemPile { pile_id } => {
            clear_unit_fields(params);
            clear_world_object_fields(params);
            params.world_selection.category = WorldSelectionCategory::ItemPile;
            params.world_selection.pile_id = Some(pile_id);
            params.building_selection.set(None);
        }
        WorldSelectionChange::ClearWorldObject => {
            clear_world_object_fields(params);
            if params.world_selection.category != WorldSelectionCategory::Units {
                params.world_selection.category = if params.selected_units.is_empty() {
                    WorldSelectionCategory::None
                } else {
                    WorldSelectionCategory::Units
                };
            }
        }
        WorldSelectionChange::ClearAll => {
            clear_unit_fields(params);
            clear_world_object_fields(params);
            params.world_selection.category = WorldSelectionCategory::None;
        }
    }

    if let Some(hud) = params.hud.as_deref_mut() {
        sync_primary_selection(hud, params.selected_units);
    }
    if let Some(revision) = params.revision.as_deref_mut() {
        revision.0 = revision.0.saturating_add(1);
    }
}

fn clear_unit_fields(params: &mut ApplyWorldSelectionParams<'_>) {
    params.selected_units.clear();
    if params.world_selection.category == WorldSelectionCategory::Units {
        params.world_selection.category = WorldSelectionCategory::None;
    }
}

fn clear_world_object_fields(params: &mut ApplyWorldSelectionParams<'_>) {
    params.world_selection.building_id = None;
    params.world_selection.doodad_id = None;
    params.world_selection.pile_id = None;
    params.building_selection.set(None);
    if matches!(
        params.world_selection.category,
        WorldSelectionCategory::Building
            | WorldSelectionCategory::Doodad
            | WorldSelectionCategory::ItemPile
    ) {
        params.world_selection.category = WorldSelectionCategory::None;
    }
}

/// Centralized stale-selection cleanup after simulation/world changes.
pub fn prune_world_selection(world: &WorldData, params: &mut ApplyWorldSelectionParams<'_>) {
    params.selected_units.prune_missing(world);
    params.selected_units.prune_dead(world);

    let mut cleared_category = false;

    match params.world_selection.category {
        WorldSelectionCategory::Units => {
            if params.selected_units.is_empty() {
                params.world_selection.category = WorldSelectionCategory::None;
                cleared_category = true;
            }
        }
        WorldSelectionCategory::Building => {
            if params
                .world_selection
                .building_id
                .is_none_or(|id| world.get_building(id).is_none())
            {
                clear_world_object_fields(params);
                cleared_category = true;
            }
        }
        WorldSelectionCategory::Doodad => {
            if params
                .world_selection
                .doodad_id
                .is_none_or(|id| world.get_doodad(id).is_none())
            {
                clear_world_object_fields(params);
                cleared_category = true;
            }
        }
        WorldSelectionCategory::ItemPile => {
            if params
                .world_selection
                .pile_id
                .is_none_or(|id| world.item_pile_store().get(id).is_none())
            {
                clear_world_object_fields(params);
                cleared_category = true;
            }
        }
        WorldSelectionCategory::None => {}
    }

    if cleared_category {
        if let Some(hud) = params.hud.as_deref_mut() {
            sync_primary_selection(hud, params.selected_units);
        }
        if let Some(revision) = params.revision.as_deref_mut() {
            revision.0 = revision.0.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests;

pub mod presentation;
