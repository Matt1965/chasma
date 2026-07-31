//! Dirty navigation-edit guards for selection and dev-mode changes (Slice 7).

use bevy::prelude::*;

use crate::client::selection::{
    ApplyWorldSelectionParams, WorldSelectionCategory, WorldSelectionChange,
    WorldSelectionRevision, WorldSelectionState, apply_world_selection,
};
use crate::dev::inspector::{
    BlueprintInspectionState, BlueprintPendingConfirmation,
    blueprint_edit_blocks_building_selection,
};
use crate::ui::gameplay::GameplayBuildingSelection;
use crate::units::input::SelectedUnits;

use super::state::{
    GuardedSelectionSnapshot, NavigationEditorBlockedAction, NavigationEditorUiState,
};

/// Revert selection changes while blueprint edits are dirty.
pub fn guard_dirty_navigation_selection(
    mut tracked_revision: Local<u64>,
    mut guarded_snapshot: Local<Option<GuardedSelectionSnapshot>>,
    mut world_selection: ResMut<WorldSelectionState>,
    mut inspection: ResMut<BlueprintInspectionState>,
    mut ui_state: ResMut<NavigationEditorUiState>,
    mut selected_units: ResMut<SelectedUnits>,
    mut building_selection: ResMut<GameplayBuildingSelection>,
    mut selection_revision: ResMut<WorldSelectionRevision>,
) {
    let revision = selection_revision.0;
    if *tracked_revision == 0 {
        *guarded_snapshot = Some(GuardedSelectionSnapshot::capture(world_selection.as_ref()));
        *tracked_revision = revision;
        return;
    }

    if revision == *tracked_revision {
        return;
    }

    let previous = guarded_snapshot
        .clone()
        .unwrap_or_else(|| GuardedSelectionSnapshot::capture(world_selection.as_ref()));

    if blueprint_edit_blocks_building_selection(&inspection) {
        let current = GuardedSelectionSnapshot::capture(world_selection.as_ref());
        if current != previous {
            let mut params = ApplyWorldSelectionParams {
                world_selection: &mut world_selection,
                selected_units: &mut selected_units,
                building_selection: &mut building_selection,
                hud: None,
                revision: Some(&mut selection_revision),
            };
            let change = snapshot_to_change(&previous);
            apply_world_selection(change, &mut params);
            inspection.pending_confirmation = Some(BlueprintPendingConfirmation::DiscardEdits {
                action: "change selection".into(),
            });
            ui_state.pending_blocked_action = Some(NavigationEditorBlockedAction::ChangeSelection);
            return;
        }
    }

    *guarded_snapshot = Some(GuardedSelectionSnapshot::capture(world_selection.as_ref()));
    *tracked_revision = revision;
}

fn snapshot_to_change(snapshot: &GuardedSelectionSnapshot) -> WorldSelectionChange {
    match snapshot.category {
        WorldSelectionCategory::Building => WorldSelectionChange::SelectBuilding {
            building_id: snapshot.building_id.expect("building id"),
        },
        WorldSelectionCategory::Doodad => WorldSelectionChange::SelectDoodad {
            doodad_id: snapshot.doodad_id.expect("doodad id"),
        },
        WorldSelectionCategory::ItemPile => WorldSelectionChange::SelectItemPile {
            pile_id: snapshot.pile_id.expect("pile id"),
        },
        WorldSelectionCategory::Units => WorldSelectionChange::ClearWorldObject,
        WorldSelectionCategory::None => WorldSelectionChange::ClearWorldObject,
    }
}
